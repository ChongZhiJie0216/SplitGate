//! Block detection (SPEC 组件 8). The school's FortiGate swaps the certificate of blocked sites
//! during the TLS handshake (phase 0 V5). While connected, every new direct TCP 443 connection with
//! a known domain gets one read-only TLS handshake of our own (Windows PowerShell + SslStream, so
//! Windows' own trust store decides); a Fortinet issuer, an untrusted chain or a failed handshake
//! makes the domain "suspected blocked". The check itself goes out direct like the original.

use std::collections::{HashSet, VecDeque};
use std::sync::{Mutex, OnceLock};
use std::thread;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::traffic::Connection;
use crate::{logs, platform, rules};

/// Hosts per PowerShell run; each one is bounded by the 3 s connect + 4 s handshake timeouts.
const BATCH: usize = 6;
/// Safety cap so a busy session cannot keep PowerShell running forever.
const MAX_PER_SESSION: usize = 300;

/// Pure ASCII PowerShell 5.1; same handshake as phase0\v5-block-probe\probe.ps1 (IPv4 only, like
/// SplitGate). One line per host: r|host|ok|issuer, r|host|cert|errors|issuer|root,
/// r|host|tls|message or r|host|tcp|message. Hosts come in SG_HOSTS, already validated.
const SCRIPT: &str = r#"
$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
function Clean([string]$s) { if (-not $s) { return '' }; return ($s -replace '[\r\n|]', ' ').Trim() }
$cb = [Net.Security.RemoteCertificateValidationCallback]{
    param($src, $cert, $chain, $errors)
    $script:errs = [string]$errors
    if ($cert) { $script:issuer = $cert.Issuer }
    if ($chain -and $chain.ChainElements.Count -gt 0) { $script:root = $chain.ChainElements[$chain.ChainElements.Count - 1].Certificate.Subject }
    return $true
}
foreach ($h in ($env:SG_HOSTS -split ' ')) {
    if (-not $h) { continue }
    $script:errs = ''; $script:issuer = ''; $script:root = ''
    $tcp = New-Object Net.Sockets.TcpClient([Net.Sockets.AddressFamily]::InterNetwork)
    $ok = $false
    try { $ok = $tcp.ConnectAsync($h, 443).Wait(3000) -and $tcp.Connected } catch { }
    if (-not $ok) { Write-Output "r|$h|tcp|no connection"; $tcp.Close(); continue }
    $tcp.ReceiveTimeout = 4000; $tcp.SendTimeout = 4000
    $ssl = New-Object Net.Security.SslStream($tcp.GetStream(), $false, $cb)
    try {
        $ssl.AuthenticateAsClient($h)
        if ($script:errs -eq 'None') { Write-Output "r|$h|ok|$(Clean $script:issuer)" }
        else { Write-Output "r|$h|cert|$(Clean $script:errs)|$(Clean $script:issuer)|$(Clean $script:root)" }
    } catch {
        $e = $_.Exception; while ($e.InnerException) { $e = $e.InnerException }
        Write-Output "r|$h|tls|$(Clean $e.Message)"
    } finally { $ssl.Dispose(); $tcp.Close() }
}
"#;

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// Certificate issued by Fortinet: the firewall replaced it.
    Fortinet,
    /// Chain not trusted by Windows (another inspecting firewall).
    Untrusted,
    /// TCP works but TLS fails: maybe a reset by the firewall, maybe the server.
    Handshake,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Blocked {
    pub host: String,
    pub reason: Reason,
    /// Issuer or error message.
    pub detail: String,
    /// Program of the connection that led to the check; empty when Windows does not tell.
    pub process_path: String,
    /// HH:MM:SS of the last check that found it.
    pub time: String,
}

#[derive(Debug, PartialEq)]
enum Outcome {
    Ok,
    Blocked(Reason, String),
    /// No conclusion (no TCP connection, name mismatch only).
    Unknown,
}

#[derive(Default)]
struct State {
    /// Hosts already queued or checked in this session.
    checked: HashSet<String>,
    queue: VecDeque<(String, String)>,
    worker: bool,
    /// Bumped by every session start / end, so a running batch from an old session is dropped.
    session: u64,
    found: Vec<Blocked>,
    /// Dismissed by the user; never listed again while the app runs.
    ignored: HashSet<String>,
}

fn state() -> &'static Mutex<State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(Default::default)
}

/// New connection: check every host again (the network may have changed). Earlier findings stay.
pub fn start_session() {
    if let Ok(mut s) = state().lock() {
        s.session += 1;
        s.checked.clear();
        s.queue.clear();
    }
}

pub fn end_session() {
    start_session();
}

/// Called with every connection snapshot; queues new direct HTTPS hosts.
pub fn observe(app: &AppHandle, connections: &[Connection]) {
    let Ok(mut s) = state().lock() else { return };
    for c in connections.iter().filter(|c| is_candidate(c)) {
        let host = c.host.to_ascii_lowercase();
        if s.checked.len() >= MAX_PER_SESSION || s.ignored.contains(&host) || !s.checked.insert(host.clone()) {
            continue;
        }
        s.queue.push_back((host, c.process_path.clone()));
    }
    if !s.queue.is_empty() && !s.worker {
        s.worker = true;
        let app = app.clone();
        thread::spawn(move || work(app));
    }
}

fn is_candidate(c: &Connection) -> bool {
    c.outbound == "direct" && c.network == "tcp" && c.port == "443" && valid_host(&c.host)
}

/// A domain name (not an IP): only these go into the script.
fn valid_host(h: &str) -> bool {
    rules::valid_domain(h) && h.rsplit('.').next().is_some_and(|tld| tld.chars().all(|c| c.is_ascii_alphabetic()))
}

fn work(app: AppHandle) {
    loop {
        let (session, batch): (u64, Vec<(String, String)>) = {
            let Ok(mut s) = state().lock() else { return };
            let n = s.queue.len().min(BATCH);
            if n == 0 {
                s.worker = false;
                return;
            }
            (s.session, s.queue.drain(..n).collect())
        };
        let hosts: Vec<&str> = batch.iter().map(|(h, _)| h.as_str()).collect();
        let results = probe(&hosts);
        let Ok(mut s) = state().lock() else { return };
        if s.session != session {
            continue;
        }
        for (host, outcome) in results {
            let process_path = batch.iter().find(|(h, _)| *h == host).map(|(_, p)| p.clone()).unwrap_or_default();
            match outcome {
                Outcome::Ok => s.found.retain(|b| b.host != host),
                Outcome::Blocked(reason, detail) if !s.ignored.contains(&host) => {
                    let (_, _, _, h, mi, sec) = platform::local_time();
                    let b = Blocked { host: host.clone(), reason, detail, process_path, time: format!("{h:02}:{mi:02}:{sec:02}") };
                    let new = !s.found.iter().any(|x| x.host == host);
                    s.found.retain(|x| x.host != host);
                    s.found.insert(0, b.clone());
                    if new {
                        logs::warn(format!("疑似被当前网络拦截：{host}（{}）", b.detail));
                    }
                    let _ = app.emit("blocked_detected", &b);
                }
                _ => {}
            }
        }
    }
}

fn probe(hosts: &[&str]) -> Vec<(String, Outcome)> {
    let out = match platform::powershell(SCRIPT).env("SG_HOSTS", hosts.join(" ")).output() {
        Ok(o) => o,
        Err(e) => {
            logs::warn(format!("拦截检测无法执行：{e}"));
            return Vec::new();
        }
    };
    String::from_utf8_lossy(&out.stdout).lines().filter_map(parse_line).filter(|(h, _)| hosts.contains(&h.as_str())).collect()
}

fn parse_line(line: &str) -> Option<(String, Outcome)> {
    let f: Vec<&str> = line.trim().split('|').collect();
    if f.len() < 3 || f[0] != "r" {
        return None;
    }
    let get = |i: usize| f.get(i).copied().unwrap_or("").to_string();
    let forti = |s: &str| s.to_ascii_lowercase().contains("forti");
    let outcome = match f[2] {
        "ok" if forti(&get(3)) => Outcome::Blocked(Reason::Fortinet, get(3)),
        "ok" => Outcome::Ok,
        "cert" => {
            let (errors, issuer, root) = (get(3), get(4), get(5));
            if forti(&issuer) || forti(&root) {
                Outcome::Blocked(Reason::Fortinet, issuer)
            } else if errors.contains("ChainErrors") {
                Outcome::Blocked(Reason::Untrusted, issuer)
            } else {
                Outcome::Unknown
            }
        }
        "tls" => Outcome::Blocked(Reason::Handshake, get(3)),
        _ => Outcome::Unknown,
    };
    Some((f[1].to_string(), outcome))
}

pub fn list() -> Vec<Blocked> {
    state().lock().map(|s| s.found.clone()).unwrap_or_default()
}

/// "忽略": drop it and never report it again while the app runs.
pub fn dismiss(host: &str) -> Vec<Blocked> {
    if let Ok(mut s) = state().lock() {
        s.found.retain(|b| b.host != host);
        s.ignored.insert(host.to_ascii_lowercase());
    }
    list()
}

/// "清空": forget the findings; hosts are checked again on the next connection.
pub fn clear() -> Vec<Blocked> {
    if let Ok(mut s) = state().lock() {
        s.found.clear();
    }
    list()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_is_ascii() {
        assert!(SCRIPT.is_ascii(), "PowerShell 5.1 script must be pure ASCII");
    }

    #[test]
    fn classifies_probe_lines() {
        let p = |l: &str| parse_line(l).unwrap().1;
        assert_eq!(p("r|www.google.com|ok|CN=WR2, O=Google Trust Services, C=US"), Outcome::Ok);
        // FortiGate deep inspection with its CA installed: trusted, still intercepted.
        assert_eq!(p("r|xbox.com|ok|CN=FortiGate CA, O=Fortinet"), Outcome::Blocked(Reason::Fortinet, "CN=FortiGate CA, O=Fortinet".into()));
        assert_eq!(
            p("r|user.auth.xboxlive.com|cert|RemoteCertificateChainErrors|CN=FG100E, O=Fortinet|CN=FG100E, O=Fortinet"),
            Outcome::Blocked(Reason::Fortinet, "CN=FG100E, O=Fortinet".into())
        );
        assert_eq!(
            p("r|steamcdn-a.akamaihd.net|cert|RemoteCertificateChainErrors|CN=Some Proxy|CN=Some Proxy"),
            Outcome::Blocked(Reason::Untrusted, "CN=Some Proxy".into())
        );
        assert_eq!(p("r|a.example.com|cert|RemoteCertificateNameMismatch|CN=R11, O=Let's Encrypt|"), Outcome::Unknown);
        assert_eq!(
            p("r|b.example.com|tls|An existing connection was forcibly closed by the remote host"),
            Outcome::Blocked(Reason::Handshake, "An existing connection was forcibly closed by the remote host".into())
        );
        assert_eq!(p("r|c.example.com|tcp|no connection"), Outcome::Unknown);
        assert!(parse_line("WARNING: something").is_none());
    }

    #[test]
    fn only_direct_https_domains_are_checked() {
        let c = |host: &str, port: &str, outbound: &str, network: &str| Connection {
            id: "x".into(),
            network: network.into(),
            host: host.into(),
            ip: String::new(),
            port: port.into(),
            process_path: String::new(),
            outbound: outbound.into(),
            rule: String::new(),
            upload: 0,
            download: 0,
            start: String::new(),
        };
        assert!(is_candidate(&c("store.steampowered.com", "443", "direct", "tcp")));
        assert!(!is_candidate(&c("store.steampowered.com", "443", "vpn", "tcp")), "already in a rule");
        assert!(!is_candidate(&c("store.steampowered.com", "80", "direct", "tcp")));
        assert!(!is_candidate(&c("store.steampowered.com", "443", "direct", "udp")), "QUIC has no certificate to swap here");
        assert!(!is_candidate(&c("", "443", "direct", "tcp")), "IP only: no SNI to test");
        assert!(!is_candidate(&c("142.250.1.1", "443", "direct", "tcp")));
        assert!(!is_candidate(&c("evil.com $(calc)", "443", "direct", "tcp")), "never reaches the script");
    }
}
