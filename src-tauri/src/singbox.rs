//! sing-box config generation (SPEC 组件 3) and the small Clash API client.
//! The config is built in memory and handed to sing-box on stdin (`-c stdin`): it holds the
//! VPN password / private key and must never be written to disk.
//! Layout follows what phase 0 verified (V1–V6): TUN with strict_route, sniff → hijack-dns →
//! IPv6 reject → process rules → address rules → vpn, everything else direct, DNS ipv4_only.

use std::io::{Read, Write};
use std::net::{IpAddr, TcpStream};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{json, Value};

use crate::config::{ExitType, Profile};
use crate::rules::RuleSet;
use crate::settings::Settings;

pub const TUN_NAME: &str = "SplitGate-TUN";
pub const VPN_TAG: &str = "vpn";
/// Fetched through the VPN to prove the tunnel carries traffic (Clash API delay test).
pub const PROBE_URL: &str = "http://cp.cloudflare.com/generate_204";
/// Used when a WireGuard .conf has no DNS line (phase 0 V2 did the same).
const WG_FALLBACK_DNS: &str = "1.1.1.1";

#[derive(Clone, serde::Deserialize)]
pub struct Secrets {
    /// Cisco password or WireGuard private key.
    pub main: String,
    pub preshared_key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ClashApi {
    pub port: u16,
    pub secret: String,
}

/// All enabled rule sets selected by the profile, merged.
#[derive(Default, Debug, PartialEq)]
pub struct MergedRules {
    pub domain_suffix: Vec<String>,
    pub domain_keyword: Vec<String>,
    pub process_path: Vec<String>,
    pub process_name: Vec<String>,
    pub ip_cidr: Vec<String>,
}

impl MergedRules {
    pub fn is_empty(&self) -> bool {
        self.domain_suffix.is_empty()
            && self.domain_keyword.is_empty()
            && self.process_path.is_empty()
            && self.process_name.is_empty()
            && self.ip_cidr.is_empty()
    }
}

/// Rule sets plus the profile's own home LAN networks (those always go through the exit).
pub fn merge_rules(profile: &Profile, rulesets: &[RuleSet]) -> MergedRules {
    let mut m = MergedRules::default();
    add_unique(&mut m.ip_cidr, &profile.home_cidrs);
    for r in rulesets.iter().filter(|r| r.enabled && profile.ruleset_ids.contains(&r.id)) {
        add_unique(&mut m.domain_suffix, &r.domain_suffix);
        add_unique(&mut m.domain_keyword, &r.domain_keyword);
        add_unique(&mut m.process_path, &r.process_path);
        add_unique(&mut m.process_name, &r.process_name);
        add_unique(&mut m.ip_cidr, &r.ip_cidr);
    }
    m
}

fn add_unique(into: &mut Vec<String>, items: &[String]) {
    for i in items {
        if !into.iter().any(|x| x.eq_ignore_ascii_case(i)) {
            into.push(i.clone());
        }
    }
}

/// "vpn.example.com", "vpn.example.com:8443" or a full URL → https URL sing-box accepts.
pub fn cisco_server_url(server: &str) -> String {
    let s = server.trim().trim_end_matches('/');
    if s.contains("://") {
        s.to_string()
    } else {
        format!("https://{s}")
    }
}

/// Host part of a URL or host:port ("[v6]:port" too).
pub fn host_of(server: &str) -> String {
    let s = server.split("://").last().unwrap_or(server);
    let s = s.split('/').next().unwrap_or(s);
    if let Some(rest) = s.strip_prefix('[') {
        return rest.split(']').next().unwrap_or(rest).to_string();
    }
    s.rsplit_once(':').map(|(h, _)| h).unwrap_or(s).to_string()
}

fn split_host_port(endpoint: &str) -> Result<(String, u16), String> {
    let port = endpoint.rsplit_once(':').and_then(|(_, p)| p.parse::<u16>().ok());
    match port {
        Some(p) => Ok((host_of(endpoint), p)),
        None => Err(format!("WireGuard Endpoint“{endpoint}”要写成 主机:端口")),
    }
}

/// The VPN endpoint plus the vpn-dns server for this exit.
fn exit_parts(profile: &Profile, secrets: &Secrets) -> Result<(Value, Value, String), String> {
    match profile.kind {
        ExitType::Cisco => {
            let c = profile.cisco.as_ref().ok_or("缺少 Cisco 设置")?;
            let mut ep = json!({
                "type": "openconnect",
                "tag": VPN_TAG,
                "server": cisco_server_url(&c.server),
                "username": c.user,
                "password": secrets.main,
            });
            if let Some(g) = &c.auth_group {
                ep["auth_group"] = json!(g);
            }
            if c.no_udp {
                ep["no_udp"] = json!(true);
            }
            if let Some(i) = &c.bind_interface {
                ep["bind_interface"] = json!(i);
            }
            // DNS servers pushed by the VPN (phase 0 V3a).
            let dns = json!({ "type": "openconnect", "tag": "vpn-dns", "endpoint": VPN_TAG, "accept_default_resolvers": true });
            Ok((ep, dns, host_of(&c.server)))
        }
        ExitType::Wireguard => {
            let w = profile.wireguard.as_ref().ok_or("缺少 WireGuard 设置")?;
            let (host, port) = split_host_port(&w.peer.endpoint)?;
            let mut peer = json!({
                "address": host,
                "port": port,
                "public_key": w.peer.public_key,
                // Rules decide what enters the tunnel, so it must accept every destination (SPEC 组件 7).
                "allowed_ips": ["0.0.0.0/0", "::/0"],
            });
            if let Some(psk) = &secrets.preshared_key {
                peer["pre_shared_key"] = json!(psk);
            }
            if let Some(k) = w.peer.persistent_keepalive {
                peer["persistent_keepalive_interval"] = json!(k);
            }
            let ep = json!({
                "type": "wireguard",
                "tag": VPN_TAG,
                "address": w.address,
                "private_key": secrets.main,
                "mtu": w.mtu,
                "peers": [peer],
            });
            let server = w
                .dns
                .iter()
                .find(|d| d.parse::<IpAddr>().is_ok_and(|ip| ip.is_ipv4()))
                .cloned()
                .unwrap_or_else(|| WG_FALLBACK_DNS.to_string());
            let dns = json!({ "type": "udp", "tag": "vpn-dns", "server": server, "detour": VPN_TAG });
            Ok((ep, dns, host))
        }
        ExitType::Pangolin => Err("Pangolin 出口第 2 阶段才能连接".into()),
    }
}

pub fn build_config(
    profile: &Profile,
    rules: &MergedRules,
    secrets: &Secrets,
    api: &ClashApi,
    settings: &Settings,
) -> Result<Value, String> {
    let (endpoint, vpn_dns, server_host) = exit_parts(profile, secrets)?;

    let mut route_rules = vec![
        json!({ "action": "sniff" }),
        json!({ "protocol": "dns", "action": "hijack-dns" }),
        // Programs that try IPv6 are refused at once and fall back to IPv4 (SPEC "网络策略").
        json!({ "ip_version": 6, "action": "reject" }),
    ];
    // Never send the VPN server itself into the VPN, even if a rule would match it.
    let server_rule = if server_host.parse::<IpAddr>().is_ok() {
        json!({ "ip_cidr": [server_host], "action": "route", "outbound": "direct" })
    } else {
        json!({ "domain": [server_host], "action": "route", "outbound": "direct" })
    };
    route_rules.push(server_rule);

    let has_process = !rules.process_path.is_empty() || !rules.process_name.is_empty();
    if has_process {
        let mut r = json!({ "action": "route", "outbound": VPN_TAG });
        if !rules.process_path.is_empty() {
            r["process_path"] = json!(rules.process_path);
        }
        if !rules.process_name.is_empty() {
            r["process_name"] = json!(rules.process_name);
        }
        route_rules.push(r);
    }
    // domain_suffix / domain_keyword / ip_cidr are OR-ed inside one sing-box rule.
    let mut addr = json!({ "action": "route", "outbound": VPN_TAG });
    let mut has_addr = false;
    for (key, list) in [("domain_suffix", &rules.domain_suffix), ("domain_keyword", &rules.domain_keyword), ("ip_cidr", &rules.ip_cidr)] {
        if !list.is_empty() {
            addr[key] = json!(list);
            has_addr = true;
        }
    }
    if has_addr {
        route_rules.push(addr);
    }

    let mut dns_rules = Vec::new();
    if !rules.domain_suffix.is_empty() || !rules.domain_keyword.is_empty() {
        let mut d = json!({ "server": "vpn-dns" });
        if !rules.domain_suffix.is_empty() {
            d["domain_suffix"] = json!(rules.domain_suffix);
        }
        if !rules.domain_keyword.is_empty() {
            d["domain_keyword"] = json!(rules.domain_keyword);
        }
        dns_rules.push(d);
    }

    let mut route = json!({
        "rules": route_rules,
        "final": "direct",
        "auto_detect_interface": true,
        "default_domain_resolver": { "server": "local" },
    });
    // Always look up the program: the Connections page shows it and "add rule" can use its path.
    route["find_process"] = json!(true);

    Ok(json!({
        "log": { "level": settings.singbox_log_level, "timestamp": true },
        "dns": {
            // "local" (system resolver) is right for Cisco / WireGuard (phase 0 V1–V6). Pangolin will need
            // the physical adapter's own DNS instead (SPEC 组件 3), added with that exit in phase 2.
            "servers": [ { "type": "local", "tag": "local" }, vpn_dns ],
            "rules": dns_rules,
            "final": "local",
            "strategy": "ipv4_only",
        },
        "inbounds": [{
            "type": "tun",
            "tag": "tun-in",
            "interface_name": TUN_NAME,
            // Keep an IPv6 address so IPv6 traffic enters the TUN (and gets rejected) instead of bypassing it.
            "address": [settings.tun_ipv4, "fdfe:dcba:9876::1/126"],
            "auto_route": true,
            "strict_route": true,
        }],
        "endpoints": [endpoint],
        "outbounds": [ { "type": "direct", "tag": "direct" } ],
        "route": route,
        "experimental": {
            "clash_api": { "external_controller": format!("127.0.0.1:{}", api.port), "secret": api.secret },
        },
    }))
}

/// `sing-box check -c stdin`. Returns sing-box's own message on failure (it never echoes secrets).
pub fn check(exe: &std::path::Path, config: &Value) -> Result<(), String> {
    let mut child = crate::platform::hidden_command(exe)
        .args(["check", "--disable-color", "-c", "stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法启动 sing-box（{}）：{e}", exe.display()))?;
    {
        let mut stdin = child.stdin.take().ok_or("sing-box stdin")?;
        stdin.write_all(config.to_string().as_bytes()).map_err(|e| format!("写入 sing-box 配置失败：{e}"))?;
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(format!("sing-box 配置检查没有通过：{msg}"))
    }
}

pub fn run_command(exe: &std::path::Path) -> Command {
    let mut cmd = crate::platform::hidden_command(exe);
    cmd.args(["run", "--disable-color", "-c", "stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    cmd
}

/// Random secret for the Clash API (loopback only, new every connection). RandomState keys come
/// from the OS random source, enough for this without another dependency.
pub fn random_secret() -> String {
    use std::hash::{BuildHasher, Hasher};
    (0..2)
        .map(|_| {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(0);
            format!("{:016x}", h.finish())
        })
        .collect()
}

/// A free loopback port for the Clash API.
pub fn free_port() -> Result<u16, String> {
    let l = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| format!("找不到空闲端口：{e}"))?;
    Ok(l.local_addr().map_err(|e| e.to_string())?.port())
}

impl ClashApi {
    /// Minimal HTTP/1.1 GET on loopback; returns (status, body).
    pub fn get(&self, path: &str, timeout: Duration) -> Result<(u16, String), String> {
        self.request("GET", path, timeout)
    }

    pub fn request(&self, method: &str, path: &str, timeout: Duration) -> Result<(u16, String), String> {
        let mut s = TcpStream::connect_timeout(&([127, 0, 0, 1], self.port).into(), Duration::from_secs(2)).map_err(|e| e.to_string())?;
        s.set_read_timeout(Some(timeout)).ok();
        let req = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n",
            self.secret
        );
        s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        parse_response(&buf)
    }

    /// Delay test through the VPN endpoint: Some(ms) when a request made it through the tunnel.
    pub fn probe_vpn(&self, timeout_ms: u32) -> Option<u32> {
        let url: String = PROBE_URL
            .bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect();
        let path = format!("/proxies/{VPN_TAG}/delay?timeout={timeout_ms}&url={url}");
        let (status, body) = self.get(&path, Duration::from_millis(timeout_ms as u64 + 2000)).ok()?;
        if status != 200 {
            return None;
        }
        serde_json::from_str::<Value>(body.trim()).ok()?["delay"].as_u64().map(|d| d as u32)
    }
}

/// (status, body) of an HTTP/1.1 response; large bodies may come chunked.
fn parse_response(buf: &[u8]) -> Result<(u16, String), String> {
    let text = String::from_utf8_lossy(buf);
    let status = text.split_whitespace().nth(1).and_then(|c| c.parse().ok()).ok_or("bad HTTP response")?;
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    if !head.to_ascii_lowercase().contains("transfer-encoding: chunked") {
        return Ok((status, body.to_string()));
    }
    let mut out = String::new();
    let mut rest = body;
    while let Some((size, after)) = rest.split_once("\r\n") {
        let n = usize::from_str_radix(size.split(';').next().unwrap_or("").trim(), 16).map_err(|_| "bad chunk")?;
        if n == 0 || after.len() < n {
            break;
        }
        out.push_str(&after[..n]);
        rest = after[n..].strip_prefix("\r\n").unwrap_or(&after[n..]);
    }
    Ok((status, out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::tests::cisco;
    use crate::config::{WireguardPeer, WireguardProfile};
    use crate::rules::presets;

    fn api() -> ClashApi {
        ClashApi { port: 9090, secret: "s".into() }
    }
    fn secrets() -> Secrets {
        Secrets { main: "synthetic-pw".into(), preshared_key: None }
    }
    fn wg_profile() -> Profile {
        let mut p = cisco("home-wg");
        p.kind = ExitType::Wireguard;
        p.cisco = None;
        p.wireguard = Some(WireguardProfile {
            address: vec!["10.8.0.2/32".into()],
            peer: WireguardPeer { endpoint: "home.example.com:51820".into(), public_key: "B".repeat(43) + "=", persistent_keepalive: Some(25) },
            dns: vec!["10.8.0.1".into()],
            mtu: 1420,
        });
        p
    }

    #[test]
    fn server_helpers() {
        assert_eq!(cisco_server_url("vpn.example.com:8443"), "https://vpn.example.com:8443");
        assert_eq!(cisco_server_url("https://vpn.example.com/"), "https://vpn.example.com");
        assert_eq!(host_of("https://vpn.example.com:8443"), "vpn.example.com");
        assert_eq!(host_of("[fd00::1]:51820"), "fd00::1");
        assert_eq!(split_host_port("home.example.com:51820").unwrap(), ("home.example.com".into(), 51820));
        assert!(split_host_port("home.example.com").is_err());
    }

    #[test]
    fn merge_only_enabled_and_selected() {
        let mut p = cisco("a");
        p.ruleset_ids = vec!["xbox".into(), "steam".into()];
        let mut sets = presets();
        sets.iter_mut().find(|r| r.id == "steam").unwrap().enabled = false;
        let m = merge_rules(&p, &sets);
        assert!(m.domain_suffix.contains(&"xboxlive.com".to_string()));
        assert!(!m.domain_suffix.iter().any(|d| d.contains("steam")), "disabled set must not apply");
        assert!(!m.domain_suffix.contains(&"minecraft.net".to_string()), "unselected set must not apply");
    }

    #[test]
    fn home_lan_goes_to_vpn_even_without_rule_sets() {
        let mut p = cisco("a");
        p.ruleset_ids = vec![];
        p.home_cidrs = vec!["192.168.1.0/24".into()];
        let m = merge_rules(&p, &presets());
        assert_eq!(m.ip_cidr, vec!["192.168.1.0/24"]);
        let c = build_config(&p, &m, &secrets(), &ClashApi { port: 9090, secret: "s".into() }, &Settings::default()).unwrap();
        let rule = c["route"]["rules"].as_array().unwrap().iter().find(|r| r["ip_cidr"] == json!(["192.168.1.0/24"])).expect("home LAN rule");
        assert_eq!(rule["outbound"], VPN_TAG);
    }

    #[test]
    fn cisco_config_layout() {
        let p = cisco("school");
        let m = MergedRules { domain_suffix: vec!["xboxlive.com".into()], process_path: vec![r"C:\G\a.exe".into()], ..Default::default() };
        let c = build_config(&p, &m, &secrets(), &api(), &Settings::default()).unwrap();
        let ep = &c["endpoints"][0];
        assert_eq!(ep["type"], "openconnect");
        assert_eq!(ep["server"], "https://vpn.example.com");
        assert_eq!(ep["auth_group"], "Master Network");
        assert_eq!(ep["no_udp"], true);
        assert!(ep.get("bind_interface").is_none());
        let rules = c["route"]["rules"].as_array().unwrap();
        let actions: Vec<String> = rules.iter().map(|r| r["action"].as_str().unwrap().to_string()).collect();
        assert_eq!(actions, vec!["sniff", "hijack-dns", "reject", "route", "route", "route"]);
        assert_eq!(rules[3]["domain"][0], "vpn.example.com", "VPN server itself goes direct");
        assert_eq!(rules[3]["outbound"], "direct");
        assert_eq!(rules[4]["process_path"][0], r"C:\G\a.exe");
        assert_eq!(rules[5]["domain_suffix"][0], "xboxlive.com");
        assert_eq!(c["route"]["find_process"], true);
        assert_eq!(c["route"]["final"], "direct");
        assert_eq!(c["dns"]["strategy"], "ipv4_only");
        assert_eq!(c["dns"]["servers"][1]["type"], "openconnect");
        assert_eq!(c["dns"]["rules"][0]["server"], "vpn-dns");
        assert_eq!(c["inbounds"][0]["strict_route"], true);
        assert_eq!(c["experimental"]["clash_api"]["external_controller"], "127.0.0.1:9090");
    }

    #[test]
    fn wireguard_config_layout() {
        let p = wg_profile();
        let s = Secrets { main: "A".repeat(43) + "=", preshared_key: Some("C".repeat(43) + "=") };
        let c = build_config(&p, &MergedRules::default(), &s, &api(), &Settings::default()).unwrap();
        let ep = &c["endpoints"][0];
        assert_eq!(ep["type"], "wireguard");
        assert_eq!(ep["peers"][0]["allowed_ips"], json!(["0.0.0.0/0", "::/0"]));
        assert_eq!(ep["peers"][0]["address"], "home.example.com");
        assert_eq!(ep["peers"][0]["port"], 51820);
        assert!(ep["peers"][0]["pre_shared_key"].is_string());
        assert_eq!(c["dns"]["servers"][1]["server"], "10.8.0.1");
        assert_eq!(c["route"]["find_process"], true, "the Connections page shows the program");
        assert_eq!(c["dns"]["rules"], json!([]));
    }

    /// Settings page values: TUN address and sing-box log level reach the config.
    fn debug_settings() -> Settings {
        Settings { singbox_log_level: "debug".into(), tun_ipv4: "10.255.0.1/24".into(), ..Settings::default() }
    }

    #[test]
    fn settings_reach_config() {
        let c = build_config(&cisco("a"), &MergedRules::default(), &secrets(), &api(), &Settings::default()).unwrap();
        assert_eq!(c["inbounds"][0]["address"][0], "172.19.0.1/30");
        assert_eq!(c["log"]["level"], "info");
        let c = build_config(&cisco("a"), &MergedRules::default(), &secrets(), &api(), &debug_settings()).unwrap();
        assert_eq!(c["inbounds"][0]["address"][0], "10.255.0.1/24");
        assert_eq!(c["log"]["level"], "debug");
    }

    #[test]
    fn http_plain_and_chunked() {
        let plain = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}";
        assert_eq!(parse_response(plain).unwrap(), (200, "{}".into()));
        let chunked = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n{\"a\"\r\n3\r\n:1}\r\n0\r\n\r\n";
        assert_eq!(parse_response(chunked).unwrap(), (200, "{\"a\":1}".into()));
        assert_eq!(parse_response(b"HTTP/1.1 204 No Content\r\n\r\n").unwrap().0, 204);
    }

    #[test]
    fn pangolin_not_yet() {
        let mut p = cisco("p");
        p.kind = ExitType::Pangolin;
        assert!(build_config(&p, &MergedRules::default(), &secrets(), &api(), &Settings::default()).is_err());
    }

    /// The generated configs must pass the pinned sing-box's own check (skipped when it is not downloaded).
    #[test]
    fn generated_configs_pass_singbox_check() {
        let exe = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../third_party/bin/sing-box/sing-box.exe");
        if !exe.exists() {
            eprintln!("skipped: {} not found (run third_party/fetch.ps1)", exe.display());
            return;
        }
        let mut p = cisco("school");
        p.ruleset_ids = vec!["xbox".into(), "minecraft".into(), "steam".into()];
        let mut sets = presets();
        sets[0].process_path = vec![r"C:\Games\Prism\javaw.exe".into()];
        sets[0].process_name = vec!["XboxPcApp.exe".into()];
        sets[0].ip_cidr = vec!["20.0.0.0/8".into()];
        sets[0].domain_keyword = vec!["xbox".into()];
        let m = merge_rules(&p, &sets);
        check(&exe, &build_config(&p, &m, &secrets(), &api(), &Settings::default()).unwrap()).unwrap();

        let w = wg_profile();
        let s = Secrets { main: "A".repeat(43) + "=", preshared_key: None };
        check(&exe, &build_config(&w, &MergedRules::default(), &s, &api(), &debug_settings()).unwrap()).unwrap();
    }
}
