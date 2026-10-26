//! settings.json: app settings (SPEC "UI 页面 · 设置"). Start with Windows is not stored here:
//! the scheduled task itself is the truth (autostart.rs). The theme is a UI-only choice kept by the
//! frontend (localStorage), so the window does not flash before the backend answers.

use std::fs;
use std::net::Ipv4Addr;

use serde::{Deserialize, Serialize};

use crate::{config, paths};

pub const SETTINGS_VERSION: u32 = 1;
pub const DEFAULT_TUN_IPV4: &str = "172.19.0.1/30";

/// The state machine reads "tunnel established" from sing-box's info lines, so nothing quieter than info.
pub const LOG_LEVELS: [&str; 3] = ["info", "debug", "trace"];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Settings {
    pub version: u32,
    /// Connect to the active profile right after the app starts.
    #[serde(default)]
    pub auto_connect: bool,
    /// After a broken connection (sleep / wake, dead tunnel, sing-box stopped): reconnect on our own.
    #[serde(default = "yes")]
    pub auto_reconnect: bool,
    /// Look for a newer GitHub release when the app starts (only a notice, nothing installed).
    #[serde(default = "yes")]
    pub check_updates: bool,
    #[serde(default = "default_level")]
    pub singbox_log_level: String,
    /// TUN adapter address with prefix, e.g. 172.19.0.1/30.
    #[serde(default = "default_tun")]
    pub tun_ipv4: String,
}

fn yes() -> bool {
    true
}
fn default_level() -> String {
    "info".into()
}
fn default_tun() -> String {
    DEFAULT_TUN_IPV4.into()
}

impl Default for Settings {
    fn default() -> Self {
        Self { version: SETTINGS_VERSION, auto_connect: false, auto_reconnect: true, check_updates: true, singbox_log_level: default_level(), tun_ipv4: default_tun() }
    }
}

/// Missing file = defaults. A broken file is an error (never overwritten silently).
pub fn load() -> Result<Settings, String> {
    let path = paths::settings_json();
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Settings::default()),
        Err(e) => return Err(format!("读取 {} 失败：{e}", path.display())),
    };
    serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("{} 格式错误（第 {} 行）：{e}。请修正或删除这个文件。", path.display(), e.line()))
}

/// For connecting: a broken or invalid file falls back to defaults, so a typo never blocks the VPN.
pub fn load_or_default() -> Settings {
    match load() {
        Ok(s) if validate(&s).is_ok() => s,
        _ => Settings::default(),
    }
}

pub fn save(s: &Settings) -> Result<(), String> {
    validate(s)?;
    config::atomic_write_json(&paths::settings_json(), s)
}

pub fn validate(s: &Settings) -> Result<(), String> {
    if !LOG_LEVELS.contains(&s.singbox_log_level.as_str()) {
        return Err(format!("sing-box 日志级别只能是 {}", LOG_LEVELS.join(" / ")));
    }
    check_tun(&s.tun_ipv4)
}

/// A private IPv4 host address with a /8–/30 prefix that is neither the network nor the broadcast address.
fn check_tun(s: &str) -> Result<(), String> {
    let bad = || format!("TUN 网段“{s}”格式不对，要写成私有 IPv4 地址/前缀，例：{DEFAULT_TUN_IPV4}");
    let (ip, len) = s.trim().split_once('/').ok_or_else(bad)?;
    let ip: Ipv4Addr = ip.parse().map_err(|_| bad())?;
    let len: u32 = len.parse().map_err(|_| bad())?;
    if !(8..=30).contains(&len) || !ip.is_private() {
        return Err(bad());
    }
    let host_bits = u32::from(ip) & ((1u32 << (32 - len)) - 1);
    if host_bits == 0 || host_bits == (1u32 << (32 - len)) - 1 {
        return Err(format!("TUN 网段“{s}”要写主机地址，不能是网络地址或广播地址，例：{DEFAULT_TUN_IPV4}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_use_defaults() {
        let s: Settings = serde_json::from_str(r#"{ "version": 1 }"#).unwrap();
        assert_eq!(s, Settings::default());
        assert!(validate(&s).is_ok());
        assert!(s.auto_reconnect, "settings.json from v0.5 / v0.6 gets automatic reconnect");
    }

    #[test]
    fn log_level_never_below_info() {
        // "tunnel established" is an info line; warn would hide it and every Cisco connect would time out.
        let mut s = Settings::default();
        for (level, ok) in [("info", true), ("debug", true), ("trace", true), ("warn", false), ("error", false)] {
            s.singbox_log_level = level.into();
            assert_eq!(validate(&s).is_ok(), ok, "{level}");
        }
    }

    #[test]
    fn tun_address_checks() {
        for (v, ok) in [
            ("172.19.0.1/30", true),
            ("10.255.0.1/24", true),
            ("192.168.200.1/30", true),
            ("172.19.0.0/30", false), // network address
            ("172.19.0.3/30", false), // broadcast
            ("8.8.8.1/30", false),    // not private
            ("172.19.0.1/31", false),
            ("172.19.0.1", false),
            ("fdfe::1/126", false),
        ] {
            assert_eq!(check_tun(v).is_ok(), ok, "{v}");
        }
    }
}
