//! Connection state machine (SPEC "连接状态机"). The frontend only shows what this reports.
//! One connect / disconnect at a time; every attempt has a generation number, so a disconnect
//! during "connecting" simply makes the old attempt stop quietly.
//!
//! Connect: cleanup leftovers → build config → `sing-box check` → `sing-box run` (stdin config,
//! inside a kill-on-close job) → wait for the tunnel (Cisco: "tunnel established") → probe through
//! the VPN (Clash API delay test) → connected. Only a successful probe counts as connected
//! (phase 0: WireGuard handshakes only on first traffic; Cisco can look up but carry nothing).

use std::io::{BufRead, BufReader, Write};
use std::process::Child;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::config::{self, ExitType};
use crate::singbox::{self, ClashApi, Secrets};
use crate::{cleanup, credentials, logs, paths, platform, rules, settings, tray};

const TUNNEL_TIMEOUT: Duration = Duration::from_secs(60);
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const PROBE_EACH_MS: u32 = 5000;

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Prepare,
    Cleanup,
    Check,
    Start,
    Tunnel,
    Probe,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Status {
    Disconnected,
    Connecting { profile_name: String, step: Step },
    Connected { profile_name: String, exit: ExitType, since_ms: u64, probe_ms: u32 },
    Disconnecting,
    Error { message: String },
}

struct Inner {
    status: Status,
    child: Option<Child>,
    generation: u64,
}

static STATE: Mutex<Inner> = Mutex::new(Inner { status: Status::Disconnected, child: None, generation: 0 });
static APP: OnceLock<AppHandle> = OnceLock::new();
#[cfg(windows)]
static JOB: OnceLock<platform::KillOnCloseJob> = OnceLock::new();

/// Events read from sing-box's stderr.
enum Ev {
    Established,
    WaitingAuth,
    ErrorLine(String),
    Exited,
}

pub fn init(app: AppHandle) {
    let _ = APP.set(app);
    #[cfg(windows)]
    match platform::KillOnCloseJob::new() {
        Ok(j) => {
            let _ = JOB.set(j);
        }
        Err(e) => logs::error(e),
    }
}

pub fn status() -> Status {
    STATE.lock().map(|s| s.status.clone()).unwrap_or(Status::Disconnected)
}

fn emit(status: &Status) {
    if let Some(app) = APP.get() {
        let _ = app.emit("status_changed", status);
        tray::update(app, status);
    }
}

/// Change the status only if `generation` is still the current attempt.
fn set_if_current(generation: u64, status: Status) -> bool {
    let Ok(mut s) = STATE.lock() else { return false };
    if s.generation != generation {
        return false;
    }
    s.status = status.clone();
    drop(s);
    emit(&status);
    true
}

fn step(generation: u64, profile_name: &str, step: Step) -> Result<(), Cancelled> {
    if set_if_current(generation, Status::Connecting { profile_name: profile_name.to_string(), step }) {
        Ok(())
    } else {
        Err(Cancelled)
    }
}

/// The attempt was superseded by a disconnect; stop without touching the status.
struct Cancelled;

enum Failure {
    Cancelled,
    Message(String),
}

impl From<String> for Failure {
    fn from(m: String) -> Self {
        Failure::Message(m)
    }
}
impl From<&str> for Failure {
    fn from(m: &str) -> Self {
        Failure::Message(m.to_string())
    }
}
impl From<Cancelled> for Failure {
    fn from(_: Cancelled) -> Self {
        Failure::Cancelled
    }
}

pub fn connect(profile_id: Option<String>) -> Result<(), String> {
    if !platform::is_admin() {
        return Err("需要以管理员身份运行 SplitGate 才能连接。".into());
    }
    let generation = {
        let mut s = STATE.lock().map_err(|_| "内部错误")?;
        if matches!(s.status, Status::Connecting { .. } | Status::Connected { .. } | Status::Disconnecting) {
            return Err("已经在连接中或已连接".into());
        }
        s.generation += 1;
        s.status = Status::Connecting { profile_name: String::new(), step: Step::Prepare };
        emit(&s.status);
        s.generation
    };
    thread::spawn(move || match run_connect(generation, profile_id) {
        Ok(()) | Err(Failure::Cancelled) => {}
        Err(Failure::Message(msg)) => {
            logs::error(format!("连接失败：{msg}"));
            let child = STATE.lock().ok().and_then(|mut s| if s.generation == generation { s.child.take() } else { None });
            stop_child(child);
            let _ = cleanup::run(true);
            logs::set_redactions(Vec::new());
            set_if_current(generation, Status::Error { message: msg });
        }
    });
    Ok(())
}

fn run_connect(generation: u64, profile_id: Option<String>) -> Result<(), Failure> {
    let (profile, rule_sets) = {
        let _guard = config::FILE_LOCK.lock().map_err(|_| "内部错误：文件锁")?;
        let file = config::load_profiles()?;
        let id = profile_id.or(file.active.clone()).ok_or("还没有 Profile。先到“Profile”页新建一个。")?;
        let p = file.profiles.into_iter().find(|p| p.id == id).ok_or("找不到这个 Profile")?;
        (p, rules::load()?.0.rulesets)
    };
    let name = profile.name.clone();
    step(generation, &name, Step::Prepare)?;
    if profile.kind == ExitType::Pangolin {
        return Err("Pangolin 出口第 2 阶段才能连接。".into());
    }
    let merged = singbox::merge_rules(&profile, &rule_sets);
    if merged.is_empty() {
        return Err("这个 Profile 没有勾选任何开着的规则集，连接了也不会有流量走 VPN。请到“Profile”页勾选规则集。".into());
    }
    let main_acc = credentials::main_account(&profile).ok_or("缺少凭据")?;
    let main = credentials::get(&main_acc)?.ok_or(match profile.kind {
        ExitType::Wireguard => "还没有保存 WireGuard 私钥。请到“Profile”页重新导入 .conf 并保存。",
        _ => "还没有保存密码。请到“Profile”页填写密码并保存。",
    })?;
    let psk = match credentials::psk_account(&profile) {
        Some(a) => credentials::get(&a)?,
        None => None,
    };
    let mut redact = vec![main.clone()];
    redact.extend(psk.clone());
    logs::set_redactions(redact);
    logs::info(format!("开始连接：{name}（{:?}）", profile.kind));

    step(generation, &name, Step::Cleanup)?;
    cleanup::run(true)?;

    step(generation, &name, Step::Check)?;
    let exe = paths::singbox_exe();
    let api = ClashApi { port: singbox::free_port()?, secret: singbox::random_secret() };
    let config = singbox::build_config(&profile, &merged, &Secrets { main, preshared_key: psk }, &api, &settings::load_or_default())?;
    singbox::check(&exe, &config)?;

    step(generation, &name, Step::Start)?;
    let mut child = singbox::run_command(&exe).spawn().map_err(|e| format!("无法启动 sing-box：{e}"))?;
    #[cfg(windows)]
    if let Some(job) = JOB.get() {
        if let Err(e) = job.assign(&child) {
            logs::warn(e);
        }
    }
    let fed = match child.stdin.take() {
        // Dropping stdin closes it: sing-box then starts parsing.
        Some(mut stdin) => stdin.write_all(config.to_string().as_bytes()).map_err(|e| format!("写入 sing-box 配置失败：{e}")),
        None => Err("sing-box stdin".into()),
    };
    drop(config);
    let rx = fed.and_then(|_| spawn_reader(&mut child));
    let rx = match rx {
        Ok(rx) => rx,
        Err(e) => {
            stop_child(Some(child));
            return Err(e.into());
        }
    };
    {
        let mut s = STATE.lock().map_err(|_| "内部错误")?;
        if s.generation != generation {
            drop(s);
            stop_child(Some(child));
            return Err(Failure::Cancelled);
        }
        s.child = Some(child);
    }

    step(generation, &name, Step::Tunnel)?;
    let mut last_error = None;
    if profile.kind == ExitType::Cisco {
        wait_established(&rx, &mut last_error)?;
    }

    step(generation, &name, Step::Probe)?;
    let probe_ms = probe(generation, &api, &rx, &mut last_error).map_err(|f| match f {
        Failure::Message(m) if m.is_empty() => Failure::Message(match profile.kind {
            ExitType::Wireguard => "WireGuard 隧道没有反应（握手没有成功）。可能被当前网络阻挡：学校网络挡 WireGuard UDP，可以换手机热点试试。".into(),
            _ => format!("隧道建立了，但流量过不去（测试网址 {} 在 30 秒内没有回应）。", singbox::PROBE_URL),
        }),
        other => other,
    })?;

    let since_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    if !set_if_current(generation, Status::Connected { profile_name: name.clone(), exit: profile.kind, since_ms, probe_ms }) {
        return Err(Failure::Cancelled);
    }
    logs::info(format!("已连接：{name}，经 VPN 测试 {probe_ms} ms"));
    thread::spawn(move || watch_exit(generation, rx));
    Ok(())
}

/// Forward every stderr line to the log; report the few lines the state machine needs.
fn spawn_reader(child: &mut Child) -> Result<Receiver<Ev>, String> {
    let stderr = child.stderr.take().ok_or("sing-box stderr")?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            logs::singbox(&line);
            let ev = if line.contains("tunnel established") {
                Some(Ev::Established)
            } else if line.contains("waiting for authentication") {
                Some(Ev::WaitingAuth)
            } else if line.contains("ERROR") || line.contains("FATAL") {
                Some(Ev::ErrorLine(line))
            } else {
                None
            };
            if let Some(ev) = ev {
                let _ = tx.send(ev);
            }
        }
        let _ = tx.send(Ev::Exited);
    });
    Ok(rx)
}

fn exited_message(last_error: &Option<String>) -> String {
    match last_error {
        Some(l) => format!("sing-box 停止了。最后的错误：{}", strip_log_prefix(l)),
        None => "sing-box 意外停止了。详细情况看“日志”页。".into(),
    }
}

/// "+0800 2026-10-26 14:55:58 ERROR [123 3ms] router: ..." → "router: ..."
fn strip_log_prefix(line: &str) -> &str {
    let after_level = line.find("ERROR").or_else(|| line.find("FATAL")).map(|i| &line[i + 5..]).unwrap_or(line);
    let s = after_level.trim_start();
    if s.starts_with('[') {
        s.split_once("] ").map(|(_, r)| r).unwrap_or(s)
    } else {
        s
    }
}

fn wait_established(rx: &Receiver<Ev>, last_error: &mut Option<String>) -> Result<(), Failure> {
    let deadline = Instant::now() + TUNNEL_TIMEOUT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(Ev::Established) => return Ok(()),
            Ok(Ev::WaitingAuth) => {
                return Err("服务器要求额外的登录信息（多半是“组别”）。请到“Profile”页填写组别，例如 Master Network。".into())
            }
            Ok(Ev::ErrorLine(l)) => *last_error = Some(l),
            Ok(Ev::Exited) | Err(RecvTimeoutError::Disconnected) => return Err(exited_message(last_error).into()),
            Err(RecvTimeoutError::Timeout) => {
                let why = last_error.as_deref().map(strip_log_prefix).unwrap_or("没有回应");
                return Err(format!("连接 Cisco 服务器超时（60 秒）：{why}").into());
            }
        }
    }
}

/// Returns the delay in ms; Failure::Message("") means "timed out" (caller words it per exit type).
fn probe(generation: u64, api: &ClashApi, rx: &Receiver<Ev>, last_error: &mut Option<String>) -> Result<u32, Failure> {
    let deadline = Instant::now() + PROBE_TIMEOUT;
    while Instant::now() < deadline {
        while let Ok(ev) = rx.try_recv() {
            match ev {
                Ev::ErrorLine(l) => *last_error = Some(l),
                Ev::Exited => return Err(exited_message(last_error).into()),
                _ => {}
            }
        }
        if STATE.lock().map(|s| s.generation != generation).unwrap_or(true) {
            return Err(Failure::Cancelled);
        }
        if let Some(ms) = api.probe_vpn(PROBE_EACH_MS) {
            return Ok(ms);
        }
        thread::sleep(Duration::from_millis(500));
    }
    Err(Failure::Message(String::new()))
}

/// After "connected": if sing-box stops on its own, clean up and show the error.
fn watch_exit(generation: u64, rx: Receiver<Ev>) {
    let mut last_error = None;
    loop {
        match rx.recv() {
            Ok(Ev::ErrorLine(l)) => last_error = Some(l),
            Ok(Ev::Exited) | Err(_) => break,
            Ok(_) => {}
        }
    }
    let child = STATE.lock().ok().and_then(|mut s| if s.generation == generation { s.child.take() } else { None });
    if child.is_none() {
        return; // a normal disconnect already took it
    }
    stop_child(child);
    let msg = exited_message(&last_error);
    logs::error(format!("连接中断：{msg}"));
    let _ = cleanup::run(true);
    logs::set_redactions(Vec::new());
    set_if_current(generation, Status::Error { message: format!("{msg} 已清理，网络已恢复。") });
}

fn stop_child(child: Option<Child>) {
    if let Some(mut c) = child {
        let _ = c.kill();
        let _ = c.wait();
    }
}

/// Stop sing-box, clean up, report "disconnected". Runs in the background.
pub fn disconnect() -> Result<(), String> {
    let Some((generation, child)) = begin_disconnect()? else { return Ok(()) };
    thread::spawn(move || finish_disconnect(generation, child));
    Ok(())
}

/// Same, but waits: used when the app is closing.
pub fn shutdown() {
    if let Ok(Some((generation, child))) = begin_disconnect() {
        finish_disconnect(generation, child);
    }
}

fn begin_disconnect() -> Result<Option<(u64, Option<Child>)>, String> {
    let mut s = STATE.lock().map_err(|_| "内部错误")?;
    match s.status {
        Status::Disconnected | Status::Disconnecting => return Ok(None),
        Status::Error { .. } if s.child.is_none() => {
            s.status = Status::Disconnected;
            emit(&s.status);
            return Ok(None);
        }
        _ => {}
    }
    s.generation += 1;
    s.status = Status::Disconnecting;
    emit(&s.status);
    Ok(Some((s.generation, s.child.take())))
}

fn finish_disconnect(generation: u64, child: Option<Child>) {
    let t = Instant::now();
    stop_child(child);
    if let Err(e) = cleanup::run(true) {
        logs::error(e);
    }
    logs::set_redactions(Vec::new());
    logs::info(format!("已断开（{} ms）", t.elapsed().as_millis()));
    set_if_current(generation, Status::Disconnected);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_singbox_log_prefix() {
        let l = "+0800 2026-10-26 14:55:58 ERROR [3123979429 3ms] router: process DNS packet: dial udp 1.1.1.1:53";
        assert_eq!(strip_log_prefix(l), "router: process DNS packet: dial udp 1.1.1.1:53");
        assert_eq!(strip_log_prefix("FATAL[0000] start service: x"), "start service: x");
    }

    #[test]
    fn status_json_shape() {
        let s = Status::Connecting { profile_name: "a".into(), step: Step::Probe };
        assert_eq!(serde_json::to_value(&s).unwrap(), serde_json::json!({"state": "connecting", "profile_name": "a", "step": "probe"}));
        assert_eq!(serde_json::to_value(Status::Disconnected).unwrap(), serde_json::json!({"state": "disconnected"}));
    }
}
