//! Diagnostics package (SPEC IPC `export_diagnostics`, 日志页“导出诊断包”): one zip with the
//! last days of logs, the settings files, the sing-box config of the active profile, the network
//! state and a summary. Everything is scrubbed before it is written: secrets become ***, and the
//! server, account, WireGuard endpoint and Windows user name become placeholders, so the zip can be
//! shared (the repo is public). Zipping uses Windows PowerShell; no extra dependency.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::config::{self, ProfilesFile};
use crate::singbox::{self, ClashApi, Secrets};
use crate::{block_detect, cleanup, credentials, logs, paths, platform, rules, settings};

const LOG_DAYS: usize = 7;
/// JSON keys whose values are always replaced, wherever they appear.
const SECRET_KEYS: [&str; 5] = ["password", "private_key", "pre_shared_key", "secret", "token"];

/// Pure ASCII PowerShell 5.1: read-only network state.
const NETWORK_SCRIPT: &str = r#"
$ErrorActionPreference = 'Continue'
"Windows: $([Environment]::OSVersion.VersionString)"
"Time: $(Get-Date -Format s)"
'--- adapters ---'
Get-NetAdapter -IncludeHidden -ErrorAction SilentlyContinue | Sort-Object ifIndex | Format-Table ifIndex, Name, InterfaceDescription, Status -AutoSize | Out-String -Width 250
'--- default routes ---'
Get-NetRoute -ErrorAction SilentlyContinue | Where-Object { $_.DestinationPrefix -in @('0.0.0.0/0', '::/0', '0.0.0.0/1', '128.0.0.0/1') } | Format-Table DestinationPrefix, NextHop, InterfaceAlias, RouteMetric -AutoSize | Out-String -Width 250
'--- routes on SplitGate adapters ---'
Get-NetRoute -ErrorAction SilentlyContinue | Where-Object { $_.InterfaceAlias -like 'SplitGate*' } | Format-Table DestinationPrefix, NextHop, InterfaceAlias -AutoSize | Out-String -Width 250
'--- DNS servers (IPv4) ---'
Get-DnsClientServerAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.ServerAddresses } | Format-Table InterfaceAlias, ServerAddresses -AutoSize | Out-String -Width 250
'--- processes ---'
Get-Process -Name 'SplitGate', 'sing-box' -ErrorAction SilentlyContinue | Format-Table Id, ProcessName, Path -AutoSize | Out-String -Width 250
"#;

/// Pure ASCII PowerShell 5.1: zip the folder SG_SRC into SG_ZIP.
const ZIP_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Compress-Archive -Path (Join-Path $env:SG_SRC '*') -DestinationPath $env:SG_ZIP -Force
"#;

/// Text to replace in every file, longest first so a server name inside a URL goes as a whole.
struct Scrubber {
    pairs: Vec<(String, String)>,
}

impl Scrubber {
    fn new(profiles: &ProfilesFile, secrets: Vec<String>) -> Self {
        let mut pairs: Vec<(String, String)> = secrets.into_iter().map(|s| (s, "***".to_string())).collect();
        for (i, p) in profiles.profiles.iter().enumerate() {
            let n = i + 1;
            if let Some(c) = &p.cisco {
                pairs.push((host_of(&c.server), format!("<cisco-server-{n}>")));
                pairs.push((c.user.clone(), format!("<cisco-user-{n}>")));
            }
            if let Some(w) = &p.wireguard {
                pairs.push((host_of(&w.peer.endpoint), format!("<wireguard-endpoint-{n}>")));
            }
        }
        if let Some(home) = std::env::var_os("USERPROFILE") {
            pairs.push((home.to_string_lossy().into_owned(), "%USERPROFILE%".into()));
        }
        if let Some(user) = std::env::var_os("USERNAME") {
            pairs.push((user.to_string_lossy().into_owned(), "<windows-user>".into()));
        }
        // Too short to replace safely (would hit unrelated text); secrets that short were never redacted either.
        pairs.retain(|(from, _)| from.len() >= 4);
        pairs.sort_by_key(|p| std::cmp::Reverse(p.0.len()));
        Self { pairs }
    }

    fn text(&self, s: &str) -> String {
        let mut out = s.to_string();
        for (from, to) in &self.pairs {
            if out.contains(from.as_str()) {
                out = out.replace(from.as_str(), to);
            }
        }
        out
    }

    fn json(&self, mut v: Value) -> String {
        mask_secret_keys(&mut v);
        self.text(&serde_json::to_string_pretty(&v).unwrap_or_default())
    }
}

/// "https://vpn.example.com:443/x" or "home.example.com:51820" → the host name.
fn host_of(s: &str) -> String {
    let s = s.split_once("://").map(|(_, r)| r).unwrap_or(s);
    let s = s.split('/').next().unwrap_or(s);
    match s.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(v6).to_string(),
        None => s.rsplit_once(':').map(|(h, _)| h).unwrap_or(s).to_string(),
    }
}

fn mask_secret_keys(v: &mut Value) {
    match v {
        Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                if SECRET_KEYS.contains(&k.as_str()) && x.is_string() {
                    *x = Value::String("***".into());
                } else {
                    mask_secret_keys(x);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(mask_secret_keys),
        _ => {}
    }
}

/// Builds the zip and returns its path (under the data folder, `diagnostics\`).
pub fn export(app_version: &str, singbox_version: Option<String>) -> Result<PathBuf, String> {
    let (y, mo, d, h, mi, s) = platform::local_time();
    let stamp = format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}");
    let dir = paths::diagnostics_dir();
    let work = dir.join(format!("tmp-{stamp}"));
    fs::create_dir_all(&work).map_err(|e| format!("无法建立文件夹 {}：{e}", work.display()))?;
    let result = collect(&work, app_version, singbox_version).and_then(|_| {
        let zip = dir.join(format!("SplitGate-diagnostics-{stamp}.zip"));
        let out = platform::powershell(ZIP_SCRIPT)
            .env("SG_SRC", &work)
            .env("SG_ZIP", &zip)
            .output()
            .map_err(|e| format!("无法执行压缩：{e}"))?;
        if out.status.success() && zip.exists() {
            Ok(zip)
        } else {
            Err(format!("压缩失败：{}", String::from_utf8_lossy(&out.stderr).trim()))
        }
    });
    let _ = fs::remove_dir_all(&work);
    if let Ok(zip) = &result {
        logs::info(format!("已导出诊断包：{}", zip.display()));
    }
    result
}

fn collect(work: &Path, app_version: &str, singbox_version: Option<String>) -> Result<(), String> {
    let profiles = config::load_profiles().unwrap_or_default();
    let scrub = Scrubber::new(&profiles, saved_secrets(&profiles));
    let write = |name: &str, text: String| fs::write(work.join(name), text).map_err(|e| format!("写入 {name} 失败：{e}"));

    // Settings files: re-serialized (so the key masking applies), not copied byte for byte.
    write("profiles.json", scrub.json(serde_json::to_value(&profiles).unwrap_or_default()))?;
    match rules::load() {
        Ok((r, _)) => write("rules.json", scrub.json(serde_json::to_value(&r).unwrap_or_default()))?,
        Err(e) => write("rules.json.error.txt", scrub.text(&e))?,
    }
    match settings::load() {
        Ok(s) => write("settings.json", scrub.json(serde_json::to_value(&s).unwrap_or_default()))?,
        Err(e) => write("settings.json.error.txt", scrub.text(&e))?,
    }
    write("singbox-config.json", singbox_config(&profiles, &scrub))?;

    let net = platform::powershell(NETWORK_SCRIPT).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    write("network.txt", scrub.text(&net.unwrap_or_else(|e| format!("无法读取：{e}"))))?;

    let logs_out = work.join("logs");
    fs::create_dir_all(&logs_out).map_err(|e| e.to_string())?;
    for f in newest_logs(&paths::logs_dir(), LOG_DAYS) {
        if let (Ok(text), Some(name)) = (fs::read_to_string(&f), f.file_name()) {
            fs::write(logs_out.join(name), scrub.text(&text)).map_err(|e| e.to_string())?;
        }
    }

    write("summary.txt", scrub.text(&summary(app_version, singbox_version)))
}

/// Every saved password / key, so even old log lines that missed redaction are caught.
fn saved_secrets(profiles: &ProfilesFile) -> Vec<String> {
    let mut out = Vec::new();
    for p in &profiles.profiles {
        for acc in [credentials::main_account(p), credentials::psk_account(p)].into_iter().flatten() {
            if let Ok(Some(s)) = credentials::get(&acc) {
                out.push(s);
            }
        }
    }
    out
}

/// What the active profile would hand to sing-box now, with *** for every secret.
fn singbox_config(profiles: &ProfilesFile, scrub: &Scrubber) -> String {
    let Some(p) = profiles.active.as_ref().and_then(|id| profiles.profiles.iter().find(|p| &p.id == id)) else {
        return "（没有使用中的 Profile）".into();
    };
    let rule_sets = rules::load().map(|(r, _)| r.rulesets).unwrap_or_default();
    let merged = singbox::merge_rules(p, &rule_sets);
    let secrets = Secrets { main: "***".into(), preshared_key: credentials::psk_account(p).map(|_| "***".into()) };
    let api = ClashApi { port: 0, secret: "***".into() };
    match singbox::build_config(p, &merged, &secrets, &api, &settings::load_or_default()) {
        Ok(v) => scrub.json(v),
        Err(e) => scrub.text(&format!("（生成失败：{e}）")),
    }
}

fn newest_logs(dir: &Path, n: usize) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "log")).collect())
        .unwrap_or_default();
    // splitgate-YYYY-MM-DD.log: the name sorts by date.
    files.sort();
    files.into_iter().rev().take(n).collect()
}

fn summary(app_version: &str, singbox_version: Option<String>) -> String {
    let leftovers = match cleanup::run(false) {
        Ok(r) if r.is_clean() => "没有".to_string(),
        Ok(r) => r.found.join("；"),
        Err(e) => e,
    };
    let blocked: Vec<String> = block_detect::list().iter().map(|b| format!("{} ({:?}: {})", b.host, b.reason, b.detail)).collect();
    [
        format!("SplitGate {app_version}"),
        format!("sing-box {}", singbox_version.unwrap_or_else(|| "（找不到）".into())),
        format!("管理员身份：{}", if platform::is_admin() { "是" } else { "否" }),
        format!("设置文件夹：{}", paths::data_dir().display()),
        format!("状态：{}", serde_json::to_string(&crate::backend::status()).unwrap_or_default()),
        format!("残留检查：{leftovers}"),
        format!("拦截提示：{}", if blocked.is_empty() { "没有".into() } else { blocked.join("；") }),
    ]
    .join("\n")
        + "\n"
}

/// Opens Explorer with the zip selected. Only files this module wrote can be shown.
pub fn show(path: &str) -> Result<(), String> {
    let p = PathBuf::from(path);
    if p.parent() != Some(paths::diagnostics_dir().as_path()) || !p.is_file() {
        return Err("找不到这个诊断包".into());
    }
    std::process::Command::new("explorer.exe").arg(format!("/select,{}", p.display())).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CiscoProfile, ExitType, Profile, WireguardPeer, WireguardProfile};

    #[test]
    fn scripts_are_ascii() {
        assert!(NETWORK_SCRIPT.is_ascii() && ZIP_SCRIPT.is_ascii(), "PowerShell 5.1 scripts must be pure ASCII");
    }

    #[test]
    fn host_names_are_extracted() {
        assert_eq!(host_of("vpn.example.com"), "vpn.example.com");
        assert_eq!(host_of("https://vpn.example.com:8443/group"), "vpn.example.com");
        assert_eq!(host_of("home.example.com:51820"), "home.example.com");
        assert_eq!(host_of("[2001:db8::1]:51820"), "2001:db8::1");
    }

    fn sample() -> ProfilesFile {
        let mut f = ProfilesFile::default();
        f.profiles.push(Profile {
            id: "a".into(),
            kind: ExitType::Cisco,
            name: "学校".into(),
            ruleset_ids: vec![],
            home_cidrs: vec![],
            cisco: Some(CiscoProfile {
                server: "https://vpn.example.com:8443".into(),
                user: "student01".into(),
                auth_group: None,
                no_udp: true,
                bind_interface: None,
                mfa: "none".into(),
            }),
            pangolin: None,
            wireguard: None,
        });
        f.profiles.push(Profile {
            id: "b".into(),
            kind: ExitType::Wireguard,
            name: "家里".into(),
            ruleset_ids: vec![],
            home_cidrs: vec![],
            cisco: None,
            pangolin: None,
            wireguard: Some(WireguardProfile {
                address: vec!["10.8.0.2/32".into()],
                peer: WireguardPeer { endpoint: "home.example.com:51820".into(), public_key: "pub".into(), persistent_keepalive: None },
                dns: vec![],
                mtu: 1420,
            }),
        });
        f
    }

    #[test]
    fn scrubs_secrets_servers_and_accounts() {
        let s = Scrubber::new(&sample(), vec!["synthetic-pw-123".into(), "ab".into()]);
        let line = "login student01 @ vpn.example.com with synthetic-pw-123; wg home.example.com:51820 ab";
        assert_eq!(s.text(line), "login <cisco-user-1> @ <cisco-server-1> with ***; wg <wireguard-endpoint-2>:51820 ab");
    }

    #[test]
    fn secret_keys_are_masked_anywhere() {
        let s = Scrubber::new(&ProfilesFile::default(), vec![]);
        let v = serde_json::json!({"endpoints": [{"type": "wireguard", "private_key": "k", "peers": [{"pre_shared_key": "p"}]}], "experimental": {"clash_api": {"secret": "x"}}, "mtu": 1420});
        let out: Value = serde_json::from_str(&s.json(v)).unwrap();
        assert_eq!(out["endpoints"][0]["private_key"], "***");
        assert_eq!(out["endpoints"][0]["peers"][0]["pre_shared_key"], "***");
        assert_eq!(out["experimental"]["clash_api"]["secret"], "***");
        assert_eq!(out["mtu"], 1420);
    }

    #[test]
    fn newest_logs_first() {
        let dir = std::env::temp_dir().join(format!("sg-diag-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        for d in ["2026-10-20", "2026-10-26", "2026-10-25"] {
            fs::write(dir.join(format!("splitgate-{d}.log")), "x").unwrap();
        }
        fs::write(dir.join("other.txt"), "x").unwrap();
        let got: Vec<String> = newest_logs(&dir, 2).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(got, vec!["splitgate-2026-10-26.log", "splitgate-2026-10-25.log"]);
        fs::remove_dir_all(&dir).unwrap();
    }
}
