//! Pangolin exit (v0.21.0, phase 0 V8): the official Pangolin client stays in charge of login and
//! its tunnel to the home site (no Exit Node). A SOCKS5 proxy runs on the home VPN-Server VM
//! (phase0/v8-pangolin-socks/server); rule traffic goes to it through the Pangolin adapter
//! (`bind_interface`). No route is added or removed. Limitation: UDP through SOCKS5 + Pangolin
//! may not work (Xbox party chat); logins, stores and game launchers are TCP.

use serde_json::json;

use super::{ExitAdapter, ExitParts};
use crate::config::PangolinProfile;
use crate::platform;
use crate::singbox::{Secrets, VPN_TAG};

/// For everything else when the physical adapter's DNS cannot be read.
const FALLBACK_LOCAL_DNS: &str = "1.1.1.1";

impl ExitAdapter for PangolinProfile {
    fn parts(&self, secrets: &Secrets) -> Result<ExitParts, String> {
        let host = self.proxy_host.trim().to_string();
        let ep = json!({
            "type": "socks",
            "tag": VPN_TAG,
            "server": host,
            "server_port": self.proxy_port,
            "version": "5",
            "username": self.proxy_user.trim(),
            "password": secrets.main,
            "bind_interface": self.interface.trim(),
        });
        let server = self.vpn_dns.first().cloned().unwrap_or_else(|| "9.9.9.9".into());
        // TCP: the safe path through SOCKS5 + Pangolin (V8).
        let dns = json!({ "type": "tcp", "tag": "vpn-dns", "server": server, "detour": VPN_TAG });
        let local_dns = platform::physical_dns(self.interface.trim()).into_iter().next().unwrap_or_else(|| FALLBACK_LOCAL_DNS.into());
        Ok(ExitParts { endpoint: ep, dns, server_host: host, is_outbound: true, local_dns: Some(local_dns) })
    }
}

/// Pure ASCII PowerShell 5.1, read-only. One line: ok | noadapter | down | route:<alias> | full.
const PRECHECK_SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$a = Get-NetAdapter -IncludeHidden -Name $env:SG_IF
if (-not $a) { 'noadapter'; exit 0 }
if ($a.Status -ne 'Up') { 'down'; exit 0 }
$r = Find-NetRoute -RemoteIPAddress $env:SG_PROXY | Where-Object { $_.InterfaceAlias } | Select-Object -First 1
if (-not $r -or $r.InterfaceAlias -ne $env:SG_IF) { "route:$($r.InterfaceAlias)"; exit 0 }
$full = @(Get-NetRoute -InterfaceAlias $env:SG_IF | Where-Object { $_.DestinationPrefix -in @('0.0.0.0/1', '128.0.0.0/1', '0.0.0.0/0') })
if ($full.Count -gt 0) { 'full'; exit 0 }
'ok'
"#;

/// Before connecting: the official client is connected and the proxy is routed through it.
/// Err stops the connect with what to do; Ok carries warnings for the connect page.
pub fn precheck(p: &PangolinProfile) -> Result<Vec<String>, String> {
    let iface = p.interface.trim();
    let out = platform::powershell(PRECHECK_SCRIPT)
        .env("SG_IF", iface)
        .env("SG_PROXY", p.proxy_host.trim())
        .output()
        .map_err(|e| format!("无法检查 Pangolin 网卡：{e}"))?;
    interpret(String::from_utf8_lossy(&out.stdout).trim(), iface, p.proxy_host.trim())
}

fn interpret(line: &str, iface: &str, proxy: &str) -> Result<Vec<String>, String> {
    match line {
        "ok" => Ok(Vec::new()),
        "noadapter" => Err(format!("找不到网卡“{iface}”：请先打开官方 Pangolin 客户端并连接（网卡名不同的话，到 Profile 页改）。")),
        "down" => Err(format!("网卡“{iface}”没有连接：请先在官方 Pangolin 客户端连接。")),
        "full" => Ok(vec![format!(
            "网卡“{iface}”上有全隧道路由，Pangolin 的 Exit Node 好像还开着：这个出口不需要它，开着的话所有流量都会经 Pangolin 出去。请在 Pangolin 后台关掉 Exit Node。"
        )]),
        l if l.starts_with("route:") => {
            let now = l.trim_start_matches("route:");
            let now = if now.is_empty() { "（没有路由）" } else { now };
            Err(format!("去家里代理 {proxy} 的流量没有经过“{iface}”（现在走 {now}）：请在 Pangolin 后台把这个 IP（或它的网段）加进 site 的资源。"))
        }
        _ => Err("检查 Pangolin 网卡失败（PowerShell 没有回应）".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> PangolinProfile {
        PangolinProfile {
            interface: "Pangolin".into(),
            proxy_host: "192.168.1.20".into(),
            proxy_port: 1080,
            proxy_user: "splitgate".into(),
            vpn_dns: vec!["9.9.9.9".into()],
        }
    }

    #[test]
    fn script_is_ascii() {
        assert!(PRECHECK_SCRIPT.is_ascii());
    }

    #[test]
    fn socks_outbound_through_the_pangolin_adapter() {
        let p = profile().parts(&Secrets { main: "synthetic-pw".into(), preshared_key: None }).unwrap();
        assert!(p.is_outbound);
        assert_eq!(p.endpoint["type"], "socks");
        assert_eq!(p.endpoint["bind_interface"], "Pangolin");
        assert_eq!(p.endpoint["server_port"], 1080);
        assert_eq!(p.endpoint["password"], "synthetic-pw");
        assert_eq!(p.dns["type"], "tcp");
        assert_eq!(p.dns["detour"], VPN_TAG);
        assert_eq!(p.server_host, "192.168.1.20");
        assert!(p.local_dns.is_some());
    }

    #[test]
    fn precheck_results() {
        assert!(interpret("ok", "Pangolin", "192.168.1.20").unwrap().is_empty());
        assert!(interpret("noadapter", "Pangolin", "x").unwrap_err().contains("官方 Pangolin 客户端"));
        assert!(interpret("route:Ethernet", "Pangolin", "192.168.1.20").unwrap_err().contains("Ethernet"));
        assert_eq!(interpret("full", "Pangolin", "x").unwrap().len(), 1, "Exit Node on: a warning, not an error");
        assert!(interpret("", "Pangolin", "x").is_err());
    }
}
