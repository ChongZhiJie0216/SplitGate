//! profiles.json: VPN connection settings without secrets (SPEC "数据模型").
//! Field names follow the SPEC (snake_case). Writes are atomic (.tmp + rename) and keep one .bak.

use std::fs;
use std::net::IpAddr;
use std::path::Path;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::paths;

pub const PROFILES_VERSION: u32 = 1;

/// Serialises every read-modify-write of the settings files.
pub static FILE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExitType {
    Cisco,
    Pangolin,
    Wireguard,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Profile {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: ExitType,
    pub name: String,
    #[serde(default)]
    pub ruleset_ids: Vec<String>,
    /// Home LAN networks (IPv4 CIDR) that always go through this exit, e.g. 192.168.1.0/24.
    #[serde(default)]
    pub home_cidrs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cisco: Option<CiscoProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pangolin: Option<PangolinProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wireguard: Option<WireguardProfile>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CiscoProfile {
    pub server: String,
    pub user: String,
    #[serde(default)]
    pub auth_group: Option<String>,
    /// TCP only: school networks block DTLS (phase 0 V3a).
    #[serde(default = "yes")]
    pub no_udp: bool,
    /// Adapter for the outer VPN connection; None = system default.
    #[serde(default)]
    pub bind_interface: Option<String>,
    /// none / duo-push / saml; MVP only supports none.
    #[serde(default = "mfa_none")]
    pub mfa: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PangolinProfile {
    #[serde(default = "pangolin_if")]
    pub interface: String,
    /// Must avoid the client's bypass list (it bypasses 1.1.1.1), phase 0 V7.
    #[serde(default = "pangolin_dns")]
    pub vpn_dns: Vec<String>,
    #[serde(default)]
    pub alias_suffixes: Vec<String>,
    #[serde(default = "pangolin_alias_dns")]
    pub alias_dns: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WireguardProfile {
    pub address: Vec<String>,
    pub peer: WireguardPeer,
    #[serde(default)]
    pub dns: Vec<String>,
    #[serde(default = "wg_mtu")]
    pub mtu: u16,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WireguardPeer {
    pub endpoint: String,
    pub public_key: String,
    #[serde(default)]
    pub persistent_keepalive: Option<u16>,
}

fn yes() -> bool {
    true
}
fn mfa_none() -> String {
    "none".into()
}
fn pangolin_if() -> String {
    "Pangolin".into()
}
fn pangolin_dns() -> Vec<String> {
    vec!["9.9.9.9".into()]
}
fn pangolin_alias_dns() -> String {
    "100.96.128.1".into()
}
fn wg_mtu() -> u16 {
    1420
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ProfilesFile {
    pub version: u32,
    #[serde(default)]
    pub active: Option<String>,
    #[serde(default)]
    pub profiles: Vec<Profile>,
}

impl Default for ProfilesFile {
    fn default() -> Self {
        Self { version: PROFILES_VERSION, active: None, profiles: Vec::new() }
    }
}

/// Missing file = empty list. A broken file is an error (never overwritten silently).
pub fn load_profiles() -> Result<ProfilesFile, String> {
    let path = paths::profiles_json();
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(ProfilesFile::default()),
        Err(e) => return Err(format!("读取 {} 失败：{e}", path.display())),
    };
    serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("{} 格式错误（第 {} 行）：{e}。请修正或删除这个文件。", path.display(), e.line()))
}

pub fn save_profiles(file: &ProfilesFile) -> Result<(), String> {
    atomic_write_json(&paths::profiles_json(), file)
}

pub fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let dir = path.parent().ok_or("无效路径")?;
    fs::create_dir_all(dir).map_err(|e| format!("建立文件夹 {} 失败：{e}", dir.display()))?;
    let mut text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    text.push('\n');
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text).map_err(|e| format!("写入 {} 失败：{e}", tmp.display()))?;
    if path.exists() {
        let _ = fs::copy(path, path.with_extension("json.bak"));
    }
    fs::rename(&tmp, path).map_err(|e| format!("保存 {} 失败：{e}", path.display()))
}

/// Checks one profile; messages are shown to the user as-is.
pub fn validate(p: &Profile) -> Result<(), String> {
    if !is_slug(&p.id) {
        return Err("内部编号无效（只能用小写字母、数字和 -）".into());
    }
    if p.name.trim().is_empty() {
        return Err("请填写名称".into());
    }
    for c in &p.home_cidrs {
        check_home_cidr(c)?;
    }
    match p.kind {
        ExitType::Cisco => {
            let c = p.cisco.as_ref().ok_or("缺少 Cisco 设置")?;
            if c.server.trim().is_empty() {
                return Err("请填写 Cisco 服务器地址".into());
            }
            if c.user.trim().is_empty() {
                return Err("请填写 Cisco 账号".into());
            }
            if c.mfa != "none" {
                return Err("这个版本只支持不需要 MFA 的服务器".into());
            }
        }
        ExitType::Pangolin => {
            let g = p.pangolin.as_ref().ok_or("缺少 Pangolin 设置")?;
            if g.interface.trim().is_empty() {
                return Err("请填写 Pangolin 网卡名称".into());
            }
            if g.vpn_dns.is_empty() {
                return Err("请至少填一个 vpn-dns".into());
            }
            for d in g.vpn_dns.iter().chain(std::iter::once(&g.alias_dns)) {
                check_ip(d)?;
            }
        }
        ExitType::Wireguard => {
            let w = p.wireguard.as_ref().ok_or("缺少 WireGuard 设置")?;
            if w.address.is_empty() {
                return Err("WireGuard 缺少 Address".into());
            }
            for a in &w.address {
                check_cidr(a)?;
            }
            for d in &w.dns {
                check_ip(d)?;
            }
            if w.peer.endpoint.trim().is_empty() || !w.peer.endpoint.contains(':') {
                return Err("WireGuard 的 Endpoint 要写成 主机:端口".into());
            }
            if w.peer.public_key.trim().len() != 44 {
                return Err("WireGuard 的 PublicKey 格式不对".into());
            }
            if !(576..=9000).contains(&w.mtu) {
                return Err("MTU 要在 576 到 9000 之间".into());
            }
        }
    }
    Ok(())
}

pub fn is_slug(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn check_ip(s: &str) -> Result<(), String> {
    s.trim().parse::<IpAddr>().map(|_| ()).map_err(|_| format!("“{s}” 不是有效的 IP 地址"))
}

pub(crate) fn check_cidr(s: &str) -> Result<(), String> {
    let (ip, len) = s.trim().split_once('/').ok_or_else(|| format!("“{s}” 要写成 IP/前缀长度"))?;
    let ip: IpAddr = ip.parse().map_err(|_| format!("“{s}” 不是有效的地址"))?;
    let max = if ip.is_ipv4() { 32 } else { 128 };
    match len.parse::<u8>() {
        Ok(n) if n <= max => Ok(()),
        _ => Err(format!("“{s}” 的前缀长度不对")),
    }
}

/// "192.168.1.0/24" → (address, prefix length).
pub fn parse_v4_cidr(s: &str) -> Option<(u32, u8)> {
    let (ip, len) = s.trim().split_once('/')?;
    let ip: std::net::Ipv4Addr = ip.parse().ok()?;
    let len: u8 = len.parse().ok().filter(|n| *n <= 32)?;
    Some((u32::from(ip), len))
}

fn v4_mask(len: u8) -> u32 {
    if len == 0 {
        0
    } else {
        u32::MAX << (32 - len)
    }
}

/// Two IPv4 networks share at least one address.
pub fn v4_overlap(a: (u32, u8), b: (u32, u8)) -> bool {
    let m = v4_mask(a.1.min(b.1));
    a.0 & m == b.0 & m
}

/// Home LAN networks: private IPv4 only, /8 to /32, written as the network address.
/// Anything wider would pull ordinary internet traffic into the VPN.
fn check_home_cidr(s: &str) -> Result<(), String> {
    let (ip, len) = parse_v4_cidr(s).ok_or_else(|| format!("家里内网网段“{s}”要写成 IPv4 网段，例如 192.168.1.0/24"))?;
    let private = [(0x0a00_0000, 8), (0xac10_0000, 12), (0xc0a8_0000, 16), (0x6440_0000, 10)];
    if len < 8 || !private.iter().any(|&net| v4_overlap((ip, len), net) && len >= net.1) {
        return Err(format!("家里内网网段“{s}”只能是内网地址（10.x、172.16–31.x、192.168.x、100.64–127.x），前缀至少 /8"));
    }
    if ip & !v4_mask(len) != 0 {
        let net = std::net::Ipv4Addr::from(ip & v4_mask(len));
        return Err(format!("家里内网网段“{s}”要写网段的第一个地址：{net}/{len}"));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn cisco(id: &str) -> Profile {
        Profile {
            id: id.into(),
            kind: ExitType::Cisco,
            name: "测试出口".into(),
            ruleset_ids: vec!["xbox".into()],
            home_cidrs: vec![],
            cisco: Some(CiscoProfile {
                server: "vpn.example.com".into(),
                user: "user01".into(),
                auth_group: Some("Master Network".into()),
                no_udp: true,
                bind_interface: None,
                mfa: "none".into(),
            }),
            pangolin: None,
            wireguard: None,
        }
    }

    #[test]
    fn spec_example_parses_with_defaults() {
        let text = r#"{ "version": 1, "active": "a", "profiles": [
            { "id": "a", "type": "cisco", "name": "C", "cisco": { "server": "s", "user": "u" } },
            { "id": "b", "type": "pangolin", "name": "P", "pangolin": {} } ] }"#;
        let f: ProfilesFile = serde_json::from_str(text).unwrap();
        let c = f.profiles[0].cisco.as_ref().unwrap();
        assert!(c.no_udp, "no_udp defaults to true (school blocks DTLS)");
        assert_eq!(c.mfa, "none");
        let g = f.profiles[1].pangolin.as_ref().unwrap();
        assert_eq!(g.interface, "Pangolin");
        assert_eq!(g.vpn_dns, vec!["9.9.9.9"]);
    }

    #[test]
    fn json_never_contains_secret_fields() {
        let text = serde_json::to_string(&cisco("a")).unwrap();
        assert!(!text.contains("password") && !text.contains("private_key"));
    }

    #[test]
    fn validation() {
        assert!(validate(&cisco("a")).is_ok());
        assert!(validate(&cisco("Bad Id")).is_err());
        let mut p = cisco("a");
        p.cisco.as_mut().unwrap().server = " ".into();
        assert!(validate(&p).is_err());
        assert!(check_cidr("10.8.0.2/32").is_ok());
        assert!(check_cidr("10.8.0.2/33").is_err());
        assert!(check_cidr("10.8.0.2").is_err());
    }

    #[test]
    fn atomic_write_keeps_backup() {
        let dir = std::env::temp_dir().join(format!("sg-cfg-{}", std::process::id()));
        let path = dir.join("profiles.json");
        let mut f = ProfilesFile::default();
        atomic_write_json(&path, &f).unwrap();
        f.active = Some("x".into());
        atomic_write_json(&path, &f).unwrap();
        let back: ProfilesFile = serde_json::from_str(&fs::read_to_string(path.with_extension("json.bak")).unwrap()).unwrap();
        assert_eq!(back.active, None);
        let now: ProfilesFile = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(now.active.as_deref(), Some("x"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn home_cidrs_must_be_private_networks() {
        for (v, ok) in [
            ("192.168.1.0/24", true),
            ("10.0.0.0/8", true),
            ("172.20.0.0/16", true),
            ("100.90.128.0/20", true),
            ("192.168.1.10/32", true),
            ("192.168.1.5/24", false), // host bits set
            ("8.8.8.0/24", false),
            ("172.32.0.0/16", false), // outside 172.16/12
            ("0.0.0.0/0", false),
            ("192.168.0.0/15", false), // wider than 192.168/16
            ("fd00::/64", false),
            ("192.168.1.0", false),
        ] {
            assert_eq!(check_home_cidr(v).is_ok(), ok, "{v}");
        }
    }

    #[test]
    fn overlap_detection() {
        let p = |s| parse_v4_cidr(s).unwrap();
        assert!(v4_overlap(p("192.168.1.0/24"), p("192.168.1.23/24")));
        assert!(v4_overlap(p("192.168.0.0/16"), p("192.168.77.5/24")));
        assert!(!v4_overlap(p("192.168.1.0/24"), p("192.168.0.10/24")));
        assert!(!v4_overlap(p("10.8.0.0/24"), p("172.20.10.2/28")));
    }
}
