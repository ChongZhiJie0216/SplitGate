//! The app's side of the background service (SPEC 组件 5, step 2, v0.17.0). When the service is
//! installed, running and points at this exe, connecting goes through it: the app reads the
//! secrets from the user's Credential Manager and sends them over the pipe; the service runs the
//! state machine and pushes status and log lines back. The app still shows connections, traffic
//! and block detection itself (from the Clash API the service reports). One pipe connection lives
//! as long as the app: when the app closes or is killed, the service disconnects.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use crate::service::{self, Event, Reply, Request};
use crate::supervisor::Status;
use crate::{config, credentials, logs, traffic, tray};

struct Session {
    writer: File,
    replies: Receiver<Reply>,
}

static APP: OnceLock<AppHandle> = OnceLock::new();
static SESSION: Mutex<Option<Session>> = Mutex::new(None);
/// The service's status as last reported; None when there is no session.
static MIRROR: Mutex<Option<Status>> = Mutex::new(None);

pub fn init(app: AppHandle) {
    let _ = APP.set(app);
}

/// A live connection to the service exists.
pub fn active() -> bool {
    SESSION.lock().map(|s| s.is_some()).unwrap_or(false)
}

/// The service's status while it handles the connection (also the "service went away" error
/// right after the pipe broke); None when the app handles connections itself.
pub fn status() -> Option<Status> {
    MIRROR.lock().ok().and_then(|m| m.clone())
}

/// Forget a broken session's last status (after the user dismissed it).
pub fn clear() {
    if let Ok(mut m) = MIRROR.lock() {
        *m = None;
    }
    show(&Status::Disconnected);
}

/// Connect through the service if it can be used; Ok(false) means "use the app itself".
/// An installed service that cannot be used is reported, not silently skipped.
pub fn try_connect(profile_id: Option<String>) -> Result<bool, String> {
    if !active() {
        let st = service::state();
        if !(st.installed && st.running && st.matches_this_exe) {
            if st.installed {
                logs::warn("后台服务装了但没有在运行，或者指向另一个位置的 SplitGate：这次由 SplitGate 自己连接");
            }
            if let Ok(mut m) = MIRROR.lock() {
                *m = None;
            }
            return Ok(false);
        }
        open()?;
    }
    let (id, main, psk) = secrets_for(profile_id)?;
    let reply = request(&Request::Connect { profile_id: id, main, preshared_key: psk })?;
    if reply.ok {
        Ok(true)
    } else {
        Err(reply.error.unwrap_or_else(|| "后台服务拒绝连接".into()))
    }
}

pub fn disconnect() -> Result<(), String> {
    let reply = request(&Request::Disconnect)?;
    if reply.ok {
        Ok(())
    } else {
        Err(reply.error.unwrap_or_else(|| "后台服务没有断开".into()))
    }
}

/// App closing: ask for a disconnect and wait a little; closing the pipe would do it anyway.
pub fn shutdown() {
    if !active() || disconnect().is_err() {
        return;
    }
    let end = Instant::now() + Duration::from_secs(10);
    while Instant::now() < end && !matches!(status(), Some(Status::Disconnected) | None) {
        thread::sleep(Duration::from_millis(200));
    }
}

/// Same rules as a local connect: the active profile when none is given; the secrets come from
/// this user's Credential Manager (the service runs as SYSTEM and cannot read it).
fn secrets_for(profile_id: Option<String>) -> Result<(String, String, Option<String>), String> {
    let profile = {
        let _guard = config::FILE_LOCK.lock().map_err(|_| "内部错误：文件锁")?;
        let file = config::load_profiles()?;
        let id = profile_id.or(file.active.clone()).ok_or("还没有 Profile。先到“Profile”页新建一个。")?;
        file.profiles.into_iter().find(|p| p.id == id).ok_or("找不到这个 Profile")?
    };
    let acc = credentials::main_account(&profile).ok_or("这个出口还不能连接")?;
    let main = credentials::get(&acc)?.ok_or(match profile.kind {
        config::ExitType::Wireguard => "还没有保存 WireGuard 私钥。请到“Profile”页重新导入 .conf 并保存。",
        _ => "还没有保存密码。请到“Profile”页填写密码并保存。",
    })?;
    let psk = match credentials::psk_account(&profile) {
        Some(a) => credentials::get(&a)?,
        None => None,
    };
    Ok((profile.id, main, psk))
}

fn open() -> Result<(), String> {
    let pipe = service::open_trusted_pipe()?;
    let reader = pipe.try_clone().map_err(|e| e.to_string())?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || read_loop(reader, tx));
    if let Ok(mut s) = SESSION.lock() {
        *s = Some(Session { writer: pipe, replies: rx });
    }
    let hello = request(&Request::Hello)?;
    logs::info(format!("经后台服务连接（服务 v{}）", hello.version.unwrap_or_default()));
    Ok(())
}

fn request(req: &Request) -> Result<Reply, String> {
    let mut guard = SESSION.lock().map_err(|_| "内部错误")?;
    let s = guard.as_mut().ok_or("没有连上后台服务")?;
    let line = serde_json::to_string(req).map_err(|e| e.to_string())?;
    s.writer.write_all(line.as_bytes()).and_then(|_| s.writer.write_all(b"\n")).and_then(|_| s.writer.flush()).map_err(|e| format!("后台服务断开了：{e}"))?;
    s.replies.recv_timeout(Duration::from_secs(15)).map_err(|_| "后台服务没有回应".to_string())
}

fn read_loop(pipe: File, replies: Sender<Reply>) {
    let mut lines = BufReader::new(pipe).lines();
    while let Some(Ok(line)) = lines.next() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        if v.get("event").is_some() {
            if let Ok(ev) = serde_json::from_value::<Event>(v) {
                on_event(ev);
            }
        } else if let Ok(r) = serde_json::from_value::<Reply>(v) {
            let _ = replies.send(r);
        }
    }
    // The service stopped or the pipe broke.
    if let Ok(mut s) = SESSION.lock() {
        *s = None;
    }
    let was = MIRROR.lock().ok().and_then(|mut m| m.take());
    if !matches!(was, None | Some(Status::Disconnected)) {
        logs::error("后台服务断开了（服务被停止或出错）；它会自己清理网络");
        show(&Status::Error { message: "后台服务断开了，连接已结束。可以按“重新连接”再试。".into(), retrying: false });
    }
}

fn on_event(ev: Event) {
    match ev {
        Event::Log { line } => logs::relay(line),
        Event::Status { status, api } => {
            if let Ok(mut m) = MIRROR.lock() {
                *m = Some(status.clone());
            }
            show(&status);
            // Connections page and traffic read the Clash API directly (it only listens on 127.0.0.1).
            if let (Status::Connected { since_ms, .. }, Some(api), Some(app)) = (&status, api, APP.get()) {
                let since = *since_ms;
                traffic::start(app.clone(), api, move || matches!(status_now(), Some(Status::Connected { since_ms, .. }) if since_ms == since));
            }
        }
    }
}

fn status_now() -> Option<Status> {
    MIRROR.lock().ok().and_then(|m| m.clone())
}

fn show(status: &Status) {
    if let Some(app) = APP.get() {
        let _ = app.emit("status_changed", status);
        tray::update(app, status);
    }
}
