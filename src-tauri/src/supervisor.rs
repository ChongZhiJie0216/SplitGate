//! Connection state machine (SPEC "连接状态机"). The frontend only shows what this reports.
//! One connect / disconnect at a time; every attempt has a generation number, so a disconnect
//! during "connecting" simply makes the old attempt stop quietly.
//!
//! Connect: cleanup leftovers → build config → `sing-box check` → `sing-box run` (stdin config,
//! inside a kill-on-close job) → wait for the tunnel (Cisco: "tunnel established") → probe through
//! the VPN (Clash API delay test) → connected. Only a successful probe counts as connected
//! (phase 0: WireGuard handshakes only on first traffic; Cisco can look up but carry nothing).
//!
//! While connected: sleep / wake is noticed and the exit is probed now and then. When the
//! connection breaks (exit silent, or sing-box stops), it cleans up first and then, if the
//! setting allows, reconnects on its own with backoff, at most 5 times (SPEC "连接状态机").

use std::io::{BufRead, BufReader, Write};
use std::process::Child;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::config::{self, ExitType};
use crate::singbox::{self, ClashApi, Secrets};
use crate::{cleanup, credentials, logs, paths, platform, rules, settings, traffic, tray};

const TUNNEL_TIMEOUT: Duration = Duration::from_secs(60);
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const PROBE_EACH_MS: u32 = 5000;

const HEALTH_TICK: Duration = Duration::from_secs(5);
const HEALTH_EVERY: Duration = Duration::from_secs(60);
/// The wall clock moved this much more than one tick: the PC was asleep.
const WAKE_GAP: Duration = Duration::from_secs(20);
/// How long the exit may stay silent before the connection counts as broken. Right after
/// waking, Wi-Fi first has to come back, so that window is longer.
const DOWN_AFTER: Duration = Duration::from_secs(20);
const DOWN_AFTER_WAKE: Duration = Duration::from_secs(60);
/// After the network changed (Wi-Fi off, cable plugged in): check at once, give it this long.
const DOWN_AFTER_CHANGE: Duration = Duration::from_secs(30);
const RETRIES: u32 = 5;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Prepare,
    Cleanup,
    Check,
    Start,
    Tunnel,
    Probe,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Status {
    Disconnected,
    Connecting { profile_name: String, step: Step },
    /// `warnings`: connected, but something will not work as expected (e.g. home LAN overlaps).
    Connected { profile_name: String, exit: ExitType, since_ms: u64, probe_ms: u32, warnings: Vec<String> },
    Disconnecting,
    /// `retrying`: the connection broke and an automatic reconnect is waiting.
    Error { message: String, retrying: bool },
}

struct Inner {
    status: Status,
    child: Option<Child>,
    generation: u64,
    /// Clash API of the running sing-box (the app's connection list reads it).
    api: Option<ClashApi>,
}

static STATE: Mutex<Inner> = Mutex::new(Inner { status: Status::Disconnected, child: None, generation: 0, api: None });
static APP: OnceLock<AppHandle> = OnceLock::new();
/// In the background service: every status change goes to the connected app through the pipe.
type Sink = Box<dyn Fn(&Status) + Send + Sync>;
static SINK: OnceLock<Sink> = OnceLock::new();
/// Secrets the app handed over for this profile (the service cannot read the user's Credential
/// Manager). Kept in memory for automatic reconnects; dropped on disconnect.
static GIVEN: Mutex<Option<(String, Secrets)>> = Mutex::new(None);
/// Set when sing-box reports a new default interface (it follows the physical network for its
/// direct traffic), so the health check does not wait for its next minute.
static NETWORK_CHANGED: AtomicBool = AtomicBool::new(false);
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
    init_job();
}

/// The background service: no window and no tray; status changes go to `sink`.
// Only the Windows background service calls this.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn init_service(sink: impl Fn(&Status) + Send + Sync + 'static) {
    let _ = SINK.set(Box::new(sink));
    init_job();
}

fn init_job() {
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
    if let Some(sink) = SINK.get() {
        sink(status);
    }
}

/// The running sing-box's Clash API while connected.
// Only the Windows background service calls this.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn clash_api() -> Option<ClashApi> {
    let s = STATE.lock().ok()?;
    matches!(s.status, Status::Connected { .. }).then(|| s.api.clone()).flatten()
}

/// The handed-over secrets are only needed while connected or reconnecting.
fn forget_given() {
    if let Ok(mut g) = GIVEN.lock() {
        *g = None;
    }
}

/// Connect with secrets supplied by the caller instead of the Credential Manager (the service).
// Only the Windows background service calls this.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn connect_with_secrets(profile_id: String, secrets: Secrets) -> Result<(), String> {
    if let Ok(mut g) = GIVEN.lock() {
        *g = Some((profile_id.clone(), secrets));
    }
    connect(Some(profile_id))
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
    let generation = begin_connect(None)?;
    thread::spawn(move || {
        let _ = attempt(generation, profile_id);
    });
    Ok(())
}

/// `only_if`: an automatic reconnect only goes ahead while nothing else happened meanwhile.
fn begin_connect(only_if: Option<u64>) -> Result<u64, String> {
    // The background service runs as SYSTEM: it has the rights, whatever IsUserAnAdmin says.
    if !platform::is_admin() && SINK.get().is_none() {
        return Err("需要以管理员身份运行 SplitGate 才能连接。".into());
    }
    let mut s = STATE.lock().map_err(|_| "内部错误")?;
    if matches!(s.status, Status::Connecting { .. } | Status::Connected { .. } | Status::Disconnecting) {
        return Err("已经在连接中或已连接".into());
    }
    if only_if.is_some_and(|g| g != s.generation) {
        return Err("已经由使用者接手".into());
    }
    s.generation += 1;
    s.status = Status::Connecting { profile_name: String::new(), step: Step::Prepare };
    emit(&s.status);
    Ok(s.generation)
}

/// One connect attempt; a failure is cleaned up and shown.
fn attempt(generation: u64, profile_id: Option<String>) -> Result<(), Failure> {
    let r = run_connect(generation, profile_id);
    if let Err(Failure::Message(msg)) = &r {
        logs::error(format!("连接失败：{msg}"));
        let child = STATE.lock().ok().and_then(|mut s| if s.generation == generation { s.child.take() } else { None });
        stop_child(child);
        let _ = cleanup::run(true);
        logs::set_redactions(Vec::new());
        set_if_current(generation, Status::Error { message: msg.clone(), retrying: false });
    }
    r
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
        return Err("Pangolin 出口还不能连接（排在整个项目最后才做）。".into());
    }
    let merged = singbox::merge_rules(&profile, &rule_sets);
    if merged.is_empty() {
        return Err("这个 Profile 没有勾选任何开着的规则集，连接了也不会有流量走 VPN。请到“Profile”页勾选规则集。".into());
    }
    let given = GIVEN.lock().ok().and_then(|g| g.as_ref().filter(|(id, _)| *id == profile.id).map(|(_, s)| s.clone()));
    let (main, psk) = match given {
        Some(s) => (s.main, s.preshared_key),
        None => {
            let main_acc = credentials::main_account(&profile).ok_or("缺少凭据")?;
            let main = credentials::get(&main_acc)?.ok_or(match profile.kind {
                ExitType::Wireguard => "还没有保存 WireGuard 私钥。请到“Profile”页重新导入 .conf 并保存。",
                _ => "还没有保存密码。请到“Profile”页填写密码并保存。",
            })?;
            let psk = match credentials::psk_account(&profile) {
                Some(a) => credentials::get(&a)?,
                None => None,
            };
            (main, psk)
        }
    };
    let mut redact = vec![main.clone()];
    redact.extend(psk.clone());
    logs::set_redactions(redact);
    logs::info(format!("开始连接：{name}（{:?}）", profile.kind));

    step(generation, &name, Step::Cleanup)?;
    cleanup::run(true)?;

    step(generation, &name, Step::Check)?;
    let warnings = home_overlaps(&profile.home_cidrs, &platform::local_ipv4_networks());
    for w in &warnings {
        logs::warn(w);
    }
    let exe = paths::singbox_exe();
    let api = ClashApi { port: singbox::free_port()?, secret: singbox::random_secret() };
    let config = singbox::build_config(&profile, &merged, &Secrets { main, preshared_key: psk }, &api, &settings::load_or_default())?;
    singbox::check(&exe, &config)?;

    step(generation, &name, Step::Start)?;
    let mut child = singbox::run_command(&exe).spawn().map_err(|e| format!("无法启动 sing-box：{e}"))?;
    #[cfg(windows)]
    match JOB.get() {
        Some(job) => match job.assign(&child) {
            Ok(()) => logs::info(format!("sing-box 已启动（pid {}），SplitGate 被强行结束时它会跟着结束", child.id())),
            Err(e) => logs::error(e),
        },
        None => logs::error("没有 Job Object：SplitGate 被强行结束时 sing-box 不会跟着结束"),
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
        s.api = Some(api.clone());
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
    if !set_if_current(generation, Status::Connected { profile_name: name.clone(), exit: profile.kind, since_ms, probe_ms, warnings }) {
        return Err(Failure::Cancelled);
    }
    logs::info(format!("已连接：{name}，经 VPN 测试 {probe_ms} ms"));
    let id = profile.id.clone();
    thread::spawn(move || watch_exit(generation, rx, id));
    // Interface lines printed while starting are not a change.
    NETWORK_CHANGED.store(false, Ordering::Relaxed);
    let (id, health_api) = (profile.id.clone(), api.clone());
    thread::spawn(move || watch_health(generation, health_api, id));
    if let Some(app) = APP.get() {
        traffic::start(app.clone(), api, move || is_connected(generation));
    }
    Ok(())
}

/// A home network that overlaps this PC's own network never reaches home: Windows keeps that
/// traffic on the local link (it is more specific than the TUN routes).
fn home_overlaps(home: &[String], local: &[(String, String)]) -> Vec<String> {
    let mut out = Vec::new();
    for h in home {
        let Some(hn) = config::parse_v4_cidr(h) else { continue };
        for (addr, adapter) in local {
            if config::parse_v4_cidr(addr).is_some_and(|ln| config::v4_overlap(hn, ln)) {
                out.push(format!(
                    "家里内网 {h} 和这台电脑现在的网络（{adapter}：{addr}）重叠：去这个网段的流量会留在本地网络，连不到家里的设备。换一个网络，或把家里的网段改成少见的（例如 192.168.77.0/24）。"
                ));
            }
        }
    }
    out
}

fn is_connected(generation: u64) -> bool {
    STATE.lock().map(|s| s.generation == generation && matches!(s.status, Status::Connected { .. })).unwrap_or(false)
}

/// Forward every stderr line to the log; report the few lines the state machine needs.
fn spawn_reader(child: &mut Child) -> Result<Receiver<Ev>, String> {
    let stderr = child.stderr.take().ok_or("sing-box stderr")?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            logs::singbox(&line);
            // "network: updated default interface Wi-Fi, index 12"
            if line.contains("default interface") {
                NETWORK_CHANGED.store(true, Ordering::Relaxed);
            }
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

/// After "connected": sing-box stopping on its own breaks the connection.
fn watch_exit(generation: u64, rx: Receiver<Ev>, profile_id: String) {
    let mut last_error = None;
    loop {
        match rx.recv() {
            Ok(Ev::ErrorLine(l)) => last_error = Some(l),
            Ok(Ev::Exited) | Err(_) => break,
            Ok(_) => {}
        }
    }
    lost(generation, profile_id, exited_message(&last_error));
}

/// While connected: notice sleep / wake (SPEC "系统睡眠后唤醒") and probe the exit every minute,
/// which also catches a dead tunnel while sing-box keeps running. Sleep shows up as the wall
/// clock jumping across one short tick, so no Windows power notification is needed.
fn watch_health(generation: u64, api: ClashApi, profile_id: String) {
    let mut last_wall = SystemTime::now();
    let mut last_check = Instant::now();
    loop {
        thread::sleep(HEALTH_TICK);
        if !is_connected(generation) {
            return;
        }
        let now = SystemTime::now();
        let gap = now.duration_since(last_wall).unwrap_or_default();
        last_wall = now;
        let changed = NETWORK_CHANGED.swap(false, Ordering::Relaxed);
        let (window, after) = if gap > HEALTH_TICK + WAKE_GAP {
            logs::info(format!("电脑从睡眠中恢复（约 {} 秒），检查 VPN 出口", gap.as_secs()));
            (DOWN_AFTER_WAKE, "睡眠唤醒后")
        } else if changed {
            logs::info("网络有变化（换了网卡或断网），检查 VPN 出口");
            (DOWN_AFTER_CHANGE, "网络有变化后")
        } else if last_check.elapsed() >= HEALTH_EVERY {
            (DOWN_AFTER, "")
        } else {
            continue;
        };
        last_check = Instant::now();
        match exit_alive(generation, &api, window) {
            Some(true) if !after.is_empty() => logs::info(format!("{after} VPN 出口正常")),
            Some(true) => {}
            Some(false) => {
                let msg = if after.is_empty() {
                    format!("VPN 出口 {} 秒内都没有回应（隧道可能断了）。", window.as_secs())
                } else {
                    format!("{after}，VPN 出口 {} 秒内都没有回应。", window.as_secs())
                };
                return lost(generation, profile_id, msg);
            }
            None => return,
        }
    }
}

/// Some(true): a probe got through; Some(false): silent for `window`; None: no longer connected.
fn exit_alive(generation: u64, api: &ClashApi, window: Duration) -> Option<bool> {
    let deadline = Instant::now() + window;
    loop {
        if !is_connected(generation) {
            return None;
        }
        if api.probe_vpn(PROBE_EACH_MS).is_some() {
            return Some(true);
        }
        if Instant::now() >= deadline {
            return Some(false);
        }
        thread::sleep(Duration::from_secs(2));
    }
}

/// The connection broke after "connected". Clean up first (the network must work again right
/// away), then reconnect on our own if the setting allows.
fn lost(generation: u64, profile_id: String, msg: String) {
    let child = STATE.lock().ok().and_then(|mut s| if s.generation == generation { s.child.take() } else { None });
    if child.is_none() {
        return; // a disconnect, or the other watcher, already took it
    }
    stop_child(child);
    logs::error(format!("连接中断：{msg}"));
    let _ = cleanup::run(true);
    logs::set_redactions(Vec::new());
    if settings::load_or_default().auto_reconnect {
        reconnect(generation, profile_id, msg);
    } else {
        set_if_current(generation, Status::Error { message: format!("{msg}已清理，网络已恢复。"), retrying: false });
    }
}

/// Backoff 2, 4, 8, 16, 32 s. Pressing connect or disconnect meanwhile takes over (generation).
fn reconnect(mut generation: u64, profile_id: String, first: String) {
    let mut why = first;
    for n in 1..=RETRIES {
        let wait = Duration::from_secs(1 << n);
        let message = format!("{why}已清理，网络已恢复。{} 秒后自动重新连接（第 {n} / {RETRIES} 次）。", wait.as_secs());
        if !set_if_current(generation, Status::Error { message, retrying: true }) || !sleep_while_current(generation, wait) {
            return;
        }
        logs::info(format!("自动重新连接（第 {n} / {RETRIES} 次）"));
        generation = match begin_connect(Some(generation)) {
            Ok(g) => g,
            Err(_) => return,
        };
        match attempt(generation, Some(profile_id.clone())) {
            Ok(()) => return logs::info(format!("自动重新连接成功（第 {n} 次）")),
            Err(Failure::Cancelled) => return,
            Err(Failure::Message(m)) => why = format!("第 {n} 次重新连接失败：{m}"),
        }
    }
    let message = format!("自动重新连接 {RETRIES} 次都没有成功（{why}）。网络已恢复，可以按“重新连接”再试。");
    forget_given();
    set_if_current(generation, Status::Error { message, retrying: false });
}

fn sleep_while_current(generation: u64, total: Duration) -> bool {
    let end = Instant::now() + total;
    while Instant::now() < end {
        if STATE.lock().map(|s| s.generation != generation).unwrap_or(true) {
            return false;
        }
        thread::sleep(Duration::from_millis(250));
    }
    STATE.lock().map(|s| s.generation == generation).unwrap_or(false)
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
            // Also stops a waiting automatic reconnect.
            forget_given();
            s.generation += 1;
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
    forget_given();
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

    #[test]
    fn home_lan_overlap_is_reported() {
        let local = vec![("192.168.1.23/24".to_string(), "Wi-Fi".to_string()), ("172.20.10.2/28".to_string(), "Ethernet".to_string())];
        let w = home_overlaps(&["192.168.1.0/24".into(), "192.168.2.0/24".into()], &local);
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("192.168.1.0/24") && w[0].contains("Wi-Fi"));
        assert!(home_overlaps(&[], &local).is_empty());
    }
}
