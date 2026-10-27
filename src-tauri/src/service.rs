//! Background Windows service (SPEC 组件 5). The same exe runs as the service with `--service`:
//! LocalSystem, automatic start. Step 1 (v0.15.0): install / remove, a named pipe that answers
//! "ping". Step 2 (v0.17.0): the connection state machine runs here (sing-box as SYSTEM, so the
//! app no longer needs administrator rights for it); the app (`remote.rs`) sends connect /
//! disconnect and the secrets, and gets status and log events back. The pipe only lets in the
//! Windows user who installed the service and Administrators.
//!
//! Protocol: one JSON object per line each way. Requests have "cmd"; replies have "ok"; events
//! (pushed after "hello") have "event". If the app that asked to connect disconnects from the
//! pipe (it was closed or killed), the service disconnects too, like the job object did before.
// The service only exists on Windows; elsewhere the shared pieces are only used by the tests.
#![cfg_attr(not(windows), allow(dead_code))]

use serde::{Deserialize, Serialize};

pub const SERVICE_NAME: &str = "SplitGateService";
pub const SERVICE_ARG: &str = "--service";
/// The installing user's SID, passed in the service command line (a SID is not a secret).
const ALLOW_SID_ARG: &str = "--allow-sid";
pub const PIPE_NAME: &str = r"\\.\pipe\SplitGate";
/// Longest request line the service reads.
const MAX_LINE: usize = 64 * 1024;

#[derive(Serialize, Debug, Clone, PartialEq, Default)]
pub struct ServiceState {
    pub installed: bool,
    pub running: bool,
    /// The service's command line (None when not installed).
    pub command: Option<String>,
    /// False when the service runs another copy (e.g. the portable folder was moved).
    pub matches_this_exe: bool,
    /// Version the service reported over the pipe; None when it did not answer.
    pub version: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub(crate) enum Request {
    Ping,
    /// Subscribe this pipe client to status and log events.
    Hello,
    /// Secrets come from the app (the service cannot read the user's Credential Manager).
    Connect {
        profile_id: String,
        main: String,
        #[serde(default)]
        preshared_key: Option<String>,
    },
    Disconnect,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub(crate) struct Reply {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Reply {
    fn ok() -> Self {
        Reply { ok: true, ..Default::default() }
    }
    fn err(e: impl Into<String>) -> Self {
        Reply { ok: false, error: Some(e.into()), ..Default::default() }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "event", rename_all = "snake_case")]
pub(crate) enum Event {
    /// `api`: the Clash API while connected, so the app can show connections and traffic itself.
    Status { status: crate::supervisor::Status, api: Option<crate::singbox::ClashApi> },
    Log { line: crate::logs::LogLine },
}

/// Requests that need no connection state; the rest are handled by the service session.
fn simple_reply(req: &Request) -> Option<Reply> {
    match req {
        Request::Ping => Some(Reply { ok: true, version: Some(env!("CARGO_PKG_VERSION").into()), error: None }),
        _ => None,
    }
}

fn parse_request(line: &str) -> Result<Request, Reply> {
    serde_json::from_str::<Request>(line).map_err(|_| Reply::err("unknown request"))
}

pub fn is_service_launch() -> bool {
    std::env::args().any(|a| a == SERVICE_ARG)
}

/// `"C:\x\SplitGate.exe" --service --allow-sid S-1-5-21-...`
fn command_line(exe: &str, sid: &str) -> String {
    format!("\"{exe}\" {SERVICE_ARG} {ALLOW_SID_ARG} {sid}")
}

/// The exe path at the start of a service command line.
fn exe_of(command: &str) -> &str {
    match command.strip_prefix('"') {
        Some(rest) => rest.split('"').next().unwrap_or(rest),
        None => command.split(' ').next().unwrap_or(command),
    }
}

/// SIDs look like S-1-5-21-1-2-3-1001: only these characters may reach the pipe's SDDL.
fn valid_sid(s: &str) -> bool {
    s.starts_with("S-1-") && s.len() < 200 && s.chars().all(|c| c.is_ascii_digit() || c == '-' || c == 'S')
}

fn allowed_sid_from_args() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == ALLOW_SID_ARG).and_then(|i| args.get(i + 1)).filter(|s| valid_sid(s)).cloned()
}

/// Pipe access: SYSTEM and Administrators always; the installing user read/write.
fn pipe_sddl(user_sid: Option<&str>) -> String {
    let mut s = "D:P(A;;GA;;;SY)(A;;GA;;;BA)".to_string();
    if let Some(sid) = user_sid.filter(|s| valid_sid(s)) {
        s.push_str(&format!("(A;;GRGW;;;{sid})"));
    }
    s
}

/// The service writes its own small log next to the app's logs (it has no window and no app log).
fn service_log(text: &str) {
    use std::io::Write;
    let (y, mo, d, h, mi, s) = crate::platform::local_time();
    let dir = crate::paths::logs_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(format!("service-{y:04}-{mo:02}-{d:02}.log"))) {
        let _ = writeln!(f, "{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02} {text}");
    }
}

#[cfg(windows)]
pub use win::{install, open_trusted_pipe, remove, run_service, start_installed, state, stop_installed, user_sid};

/// From the app: directly when it runs as administrator, otherwise in an elevated copy of
/// SplitGate (Windows shows its UAC prompt) that installs for this user (v0.18.0).
pub fn install_from_app() -> Result<ServiceState, String> {
    if crate::platform::is_admin() {
        return install(None);
    }
    crate::elevate::run(&crate::elevate::Task::InstallService { sid: user_sid()? })?;
    Ok(state())
}

pub fn remove_from_app() -> Result<ServiceState, String> {
    if crate::platform::is_admin() {
        return remove();
    }
    crate::elevate::run(&crate::elevate::Task::RemoveService)?;
    Ok(state())
}

/// The service can connect for this copy of SplitGate (no administrator rights needed then).
pub fn usable() -> bool {
    let s = state();
    s.installed && s.running && s.matches_this_exe
}

#[cfg(not(windows))]
pub fn run_service() {}
#[cfg(not(windows))]
pub fn state() -> ServiceState {
    ServiceState::default()
}
#[cfg(not(windows))]
pub fn user_sid() -> Result<String, String> {
    Err("后台服务只支持 Windows".into())
}
#[cfg(not(windows))]
pub fn install(_sid: Option<String>) -> Result<ServiceState, String> {
    Err("后台服务只支持 Windows".into())
}
#[cfg(not(windows))]
pub fn remove() -> Result<ServiceState, String> {
    Err("后台服务只支持 Windows".into())
}
#[cfg(not(windows))]
pub fn stop_installed() -> Result<bool, String> {
    Ok(false)
}
#[cfg(not(windows))]
pub fn start_installed() -> Result<(), String> {
    Ok(())
}
#[cfg(not(windows))]
pub fn open_trusted_pipe() -> Result<std::fs::File, String> {
    Err("后台服务只支持 Windows".into())
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::fs::{File, OpenOptions};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use std::ptr::{null, null_mut};
    use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
    use std::sync::{mpsc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, LocalFree, ERROR_CALL_NOT_IMPLEMENTED, ERROR_PIPE_CONNECTED, ERROR_SERVICE_ALREADY_RUNNING,
        HANDLE, INVALID_HANDLE_VALUE, NO_ERROR,
    };
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER};
    use windows_sys::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeServerProcessId, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES,
        PIPE_WAIT,
    };
    use windows_sys::Win32::System::Services::{
        ChangeServiceConfig2W, ChangeServiceConfigW, CloseServiceHandle, ControlService, CreateServiceW, DeleteService, OpenSCManagerW,
        OpenServiceW, QueryServiceConfigW, QueryServiceStatus, QueryServiceStatusEx, SC_STATUS_PROCESS_INFO, SERVICE_STATUS_PROCESS, RegisterServiceCtrlHandlerExW, SetServiceStatus, StartServiceCtrlDispatcherW,
        StartServiceW, QUERY_SERVICE_CONFIGW, SC_HANDLE, SC_MANAGER_CONNECT, SC_MANAGER_CREATE_SERVICE, SERVICE_ACCEPT_SHUTDOWN,
        SERVICE_ACCEPT_STOP, SERVICE_ALL_ACCESS, SERVICE_AUTO_START, SERVICE_CONFIG_DESCRIPTION, SERVICE_CONTROL_INTERROGATE,
        SERVICE_CONTROL_SHUTDOWN, SERVICE_CONTROL_STOP, SERVICE_DESCRIPTIONW, SERVICE_ERROR_NORMAL, SERVICE_NO_CHANGE,
        SERVICE_QUERY_CONFIG, SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_START, SERVICE_START_PENDING, SERVICE_STATUS,
        SERVICE_STATUS_CURRENT_STATE, SERVICE_STATUS_HANDLE, SERVICE_STOP, SERVICE_STOPPED, SERVICE_STOP_PENDING,
        SERVICE_TABLE_ENTRYW, SERVICE_WIN32_OWN_PROCESS,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    use super::*;
    use crate::platform;

    /// DELETE standard access right (winnt.h).
    const DELETE: u32 = 0x0001_0000;
    /// ERROR_PIPE_BUSY: every instance is serving someone; try again shortly.
    const ERROR_PIPE_BUSY: i32 = 231;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    // ---------- the service process ----------

    static STATUS_HANDLE: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
    static STOP: AtomicBool = AtomicBool::new(false);

    /// Entry point for `SplitGate.exe --service`; returns when the service has stopped.
    pub fn run_service() {
        let name = wide(SERVICE_NAME);
        let table = [
            SERVICE_TABLE_ENTRYW { lpServiceName: name.as_ptr() as *mut u16, lpServiceProc: Some(service_main) },
            SERVICE_TABLE_ENTRYW { lpServiceName: null_mut(), lpServiceProc: None },
        ];
        // Blocks until service_main returns. Fails when started by hand instead of by Windows.
        if unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) } == 0 {
            service_log("不是由 Windows 服务管理器启动的（--service 只给后台服务用），退出");
        }
    }

    fn set_state(state: SERVICE_STATUS_CURRENT_STATE) {
        let accepted = if state == SERVICE_RUNNING { SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN } else { 0 };
        let status = SERVICE_STATUS {
            dwServiceType: SERVICE_WIN32_OWN_PROCESS,
            dwCurrentState: state,
            dwControlsAccepted: accepted,
            dwWin32ExitCode: NO_ERROR,
            dwServiceSpecificExitCode: 0,
            dwCheckPoint: 0,
            dwWaitHint: if state == SERVICE_RUNNING || state == SERVICE_STOPPED { 0 } else { 5000 },
        };
        let h: SERVICE_STATUS_HANDLE = STATUS_HANDLE.load(Ordering::SeqCst);
        if !h.is_null() {
            unsafe { SetServiceStatus(h, &status) };
        }
    }

    unsafe extern "system" fn service_main(_argc: u32, _argv: *mut windows_sys::core::PWSTR) {
        let name = wide(SERVICE_NAME);
        let h = RegisterServiceCtrlHandlerExW(name.as_ptr(), Some(control_handler), null());
        if h.is_null() {
            return;
        }
        STATUS_HANDLE.store(h, Ordering::SeqCst);
        set_state(SERVICE_START_PENDING);
        let sid = allowed_sid_from_args();
        service_log(&format!("后台服务启动（v{}），管道允许的用户：{}", env!("CARGO_PKG_VERSION"), sid.as_deref().unwrap_or("只有管理员")));
        // Release builds abort on a panic with no trace anywhere; leave one in the service log.
        std::panic::set_hook(Box::new(|info| service_log(&format!("后台服务出错退出：{info}"))));
        // The app's log file and the pipe get every line; status changes go to the pipe.
        crate::logs::init_service(|line| broadcast(&Event::Log { line: line.clone() }));
        crate::supervisor::init_service(|status| {
            // Only a Connected status needs the API; it is emitted without the state lock held.
            let api = matches!(status, crate::supervisor::Status::Connected { .. }).then(crate::supervisor::clash_api).flatten();
            broadcast(&Event::Status { status: status.clone(), api });
        });
        // Leftovers of a crash (SPEC "错误处理与清理"), as the app does at startup.
        thread::spawn(|| match crate::cleanup::run(true) {
            Ok(r) if r.is_clean() => crate::logs::info("后台服务启动检查：没有残留"),
            Ok(r) => service_log(&format!("启动检查发现残留：{}；已清理：{}", r.found.join("；"), r.done.join("；"))),
            Err(e) => crate::logs::error(e),
        });
        thread::spawn(move || pipe_server(sid));
        set_state(SERVICE_RUNNING);
        while !STOP.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(200));
        }
        // Stopping (or Windows shutting down) must leave the network as it was.
        crate::supervisor::shutdown();
        service_log("后台服务停止");
        set_state(SERVICE_STOPPED);
    }

    unsafe extern "system" fn control_handler(control: u32, _event: u32, _data: *mut c_void, _ctx: *mut c_void) -> u32 {
        match control {
            SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN => {
                set_state(SERVICE_STOP_PENDING);
                STOP.store(true, Ordering::SeqCst);
                NO_ERROR
            }
            SERVICE_CONTROL_INTERROGATE => NO_ERROR,
            _ => ERROR_CALL_NOT_IMPLEMENTED,
        }
    }

    fn pipe_server(user_sid: Option<String>) {
        let sddl = wide(&pipe_sddl(user_sid.as_deref()));
        let mut sd: PSECURITY_DESCRIPTOR = null_mut();
        if unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), SDDL_REVISION_1, &mut sd, null_mut()) } == 0 {
            service_log("建立管道权限失败，管道不开放");
            return;
        }
        let sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd,
            bInheritHandle: 0,
        };
        let name = wide(PIPE_NAME);
        while !STOP.load(Ordering::SeqCst) {
            let h = unsafe {
                CreateNamedPipeW(
                    name.as_ptr(),
                    PIPE_ACCESS_DUPLEX,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                    PIPE_UNLIMITED_INSTANCES,
                    4096,
                    4096,
                    0,
                    &sa,
                )
            };
            if h == INVALID_HANDLE_VALUE {
                service_log(&format!("建立管道失败（{}），1 秒后再试", unsafe { GetLastError() }));
                thread::sleep(Duration::from_secs(1));
                continue;
            }
            let connected = unsafe { ConnectNamedPipe(h, null_mut()) } != 0 || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
            if !connected {
                unsafe { CloseHandle(h) };
                continue;
            }
            // HANDLE is a raw pointer (not Send); hand it over as a number.
            let raw = h as usize;
            thread::spawn(move || serve(raw as HANDLE));
        }
        unsafe { LocalFree(sd) };
    }

    /// Event connections (they sent "hello"): the state machine only puts lines into a channel,
    /// so a slow or stuck app can never block it.
    static CLIENTS: Mutex<Vec<(u64, mpsc::Sender<String>)>> = Mutex::new(Vec::new());
    static NEXT_CLIENT: AtomicU64 = AtomicU64::new(1);
    /// The connection that asked to connect; when it goes away, the VPN connection goes too.
    static OWNER: AtomicU64 = AtomicU64::new(0);

    fn broadcast(ev: &Event) {
        let Ok(line) = serde_json::to_string(ev) else { return };
        if let Ok(mut c) = CLIENTS.lock() {
            c.retain(|(_, tx)| tx.send(line.clone()).is_ok());
        }
    }

    fn write_line(mut file: &File, line: &str) -> std::io::Result<()> {
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        file.flush()
    }

    /// One connection, one thread. The pipe handle is synchronous, and Windows queues I/O on one
    /// handle: a write waits until a pending read on the same handle finishes. So a connection is
    /// either a command connection (read a request, write its reply, repeat) or, after "hello",
    /// an event connection (only writes from then on). v0.17.0 had a reader and a writer thread on
    /// one connection, and every reply hung behind the next read ("后台服务没有回应").
    fn serve(h: HANDLE) {
        let file = unsafe { File::from_raw_handle(h) };
        let id = NEXT_CLIENT.fetch_add(1, Ordering::SeqCst);
        let mut reader = BufReader::new(&file);
        let mut line = String::new();
        loop {
            line.clear();
            // Each line is capped on its own: a long-lived app connection never runs out.
            match (&mut reader).take(MAX_LINE as u64).read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) if !line.ends_with('\n') => break, // longer than MAX_LINE
                Ok(_) => {
                    let reply = match parse_request(line.trim()) {
                        Ok(Request::Hello) => {
                            stream_events(&file, id);
                            break;
                        }
                        Ok(req) => answer(req, id),
                        Err(r) => r,
                    };
                    if write_line(&file, &serde_json::to_string(&reply).unwrap_or_default()).is_err() {
                        break;
                    }
                }
            }
        }
        if OWNER.compare_exchange(id, 0, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            crate::logs::info("SplitGate 关掉了或被结束了，后台服务断开连接");
            service_log("SplitGate 关掉了或被结束了，断开连接");
            let r = crate::supervisor::disconnect();
            service_log(&match r {
                Ok(()) => "已断开".to_string(),
                Err(e) => format!("断开失败：{e}"),
            });
        }
    }

    /// The current state first, then every change, until the app goes away (a write fails).
    fn stream_events(file: &File, id: u64) {
        let hello = Reply { ok: true, version: Some(env!("CARGO_PKG_VERSION").into()), error: None };
        let status = crate::supervisor::status();
        let api = crate::supervisor::clash_api();
        let first = [serde_json::to_string(&hello), serde_json::to_string(&Event::Status { status, api })];
        if first.iter().flatten().any(|l| write_line(file, l).is_err()) {
            return;
        }
        let (tx, rx) = mpsc::channel::<String>();
        if let Ok(mut c) = CLIENTS.lock() {
            c.push((id, tx));
        }
        loop {
            // A quiet heartbeat (the app ignores "{}") notices an app that went away.
            let line = match rx.recv_timeout(Duration::from_secs(30)) {
                Ok(l) => l,
                Err(mpsc::RecvTimeoutError::Timeout) => "{}".to_string(),
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            if write_line(file, &line).is_err() {
                break;
            }
        }
        if let Ok(mut c) = CLIENTS.lock() {
            c.retain(|(cid, _)| *cid != id);
        }
    }

    fn answer(req: Request, id: u64) -> Reply {
        if let Some(r) = simple_reply(&req) {
            return r;
        }
        match req {
            Request::Connect { profile_id, main, preshared_key } => {
                OWNER.store(id, Ordering::SeqCst);
                service_log(&format!("收到连接请求（连接 #{id}）"));
                let secrets = crate::singbox::Secrets { main, preshared_key };
                match crate::supervisor::connect_with_secrets(profile_id, secrets) {
                    Ok(()) => Reply::ok(),
                    Err(e) => Reply::err(e),
                }
            }
            Request::Disconnect => {
                service_log("收到断开请求");
                match crate::supervisor::disconnect() {
                    Ok(()) => Reply::ok(),
                    Err(e) => Reply::err(e),
                }
            }
            Request::Ping | Request::Hello => Reply::ok(),
        }
    }

    // ---------- the app side ----------

    /// Asks the service for its version; fails fast when it is not running.
    pub fn ping() -> Result<String, String> {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(ping_blocking());
        });
        rx.recv_timeout(Duration::from_secs(3)).unwrap_or_else(|_| Err("后台服务没有回应".into()))
    }

    fn ping_blocking() -> Result<String, String> {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut pipe = loop {
            match OpenOptions::new().read(true).write(true).open(PIPE_NAME) {
                Ok(f) => break f,
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && Instant::now() < deadline => thread::sleep(Duration::from_millis(100)),
                Err(e) => return Err(format!("连不上后台服务：{e}")),
            }
        };
        pipe.write_all(b"{\"cmd\":\"ping\"}\n").map_err(|e| e.to_string())?;
        let mut line = String::new();
        BufReader::new(pipe.take(MAX_LINE as u64)).read_line(&mut line).map_err(|e| e.to_string())?;
        let r: Reply = serde_json::from_str(line.trim()).map_err(|_| "后台服务的回应看不懂".to_string())?;
        r.version.filter(|_| r.ok).ok_or_else(|| r.error.unwrap_or_else(|| "后台服务回应错误".into()))
    }

    struct Sc(SC_HANDLE);
    impl Drop for Sc {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CloseServiceHandle(self.0) };
            }
        }
    }

    fn manager(access: u32) -> Result<Sc, String> {
        let h = unsafe { OpenSCManagerW(null(), null(), access) };
        if h.is_null() {
            Err(format!("打不开 Windows 服务管理器（{}）", unsafe { GetLastError() }))
        } else {
            Ok(Sc(h))
        }
    }

    fn open(access: u32) -> Option<Sc> {
        let m = manager(SC_MANAGER_CONNECT).ok()?;
        let name = wide(SERVICE_NAME);
        let h = unsafe { OpenServiceW(m.0, name.as_ptr(), access) };
        (!h.is_null()).then_some(Sc(h))
    }

    fn current_state(svc: &Sc) -> Option<SERVICE_STATUS_CURRENT_STATE> {
        let mut st: SERVICE_STATUS = unsafe { std::mem::zeroed() };
        (unsafe { QueryServiceStatus(svc.0, &mut st) } != 0).then_some(st.dwCurrentState)
    }

    fn command(svc: &Sc) -> Option<String> {
        let mut need = 0u32;
        unsafe { QueryServiceConfigW(svc.0, null_mut(), 0, &mut need) };
        if need == 0 {
            return None;
        }
        // u64 buffer: QUERY_SERVICE_CONFIGW needs pointer alignment.
        let mut buf = vec![0u64; (need as usize).div_ceil(8)];
        let cfg = buf.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW;
        if unsafe { QueryServiceConfigW(svc.0, cfg, need, &mut need) } == 0 {
            return None;
        }
        let p = unsafe { (*cfg).lpBinaryPathName };
        (!p.is_null()).then(|| unsafe { from_wide_ptr(p) })
    }

    unsafe fn from_wide_ptr(p: *const u16) -> String {
        let mut n = 0;
        while *p.add(n) != 0 {
            n += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
    }

    fn this_exe() -> String {
        std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default()
    }

    /// The process id Windows' service manager gives for the running service.
    fn service_pid() -> Option<u32> {
        let svc = open(SERVICE_QUERY_STATUS)?;
        let mut st: SERVICE_STATUS_PROCESS = unsafe { std::mem::zeroed() };
        let mut need = 0u32;
        let size = std::mem::size_of::<SERVICE_STATUS_PROCESS>() as u32;
        let ok = unsafe { QueryServiceStatusEx(svc.0, SC_STATUS_PROCESS_INFO, &mut st as *mut _ as *mut u8, size, &mut need) } != 0;
        (ok && st.dwCurrentState == SERVICE_RUNNING && st.dwProcessId != 0).then_some(st.dwProcessId)
    }

    /// Opens the pipe and makes sure the other end really is SplitGateService: any program can
    /// create a pipe with this name first, and it must never get the secrets the app sends.
    pub fn open_trusted_pipe() -> Result<File, String> {
        let expected = service_pid().ok_or("后台服务没有在运行")?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let pipe = loop {
            match OpenOptions::new().read(true).write(true).open(PIPE_NAME) {
                Ok(f) => break f,
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && Instant::now() < deadline => thread::sleep(Duration::from_millis(100)),
                Err(e) => return Err(format!("连不上后台服务：{e}")),
            }
        };
        let mut pid = 0u32;
        let ok = unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle() as HANDLE, &mut pid) } != 0;
        if !ok || pid != expected {
            crate::logs::error(format!("管道 {PIPE_NAME} 的另一端不是 SplitGate 后台服务（进程 {pid}，服务是 {expected}），不使用"));
            return Err("连到的不是 SplitGate 后台服务".into());
        }
        Ok(pipe)
    }

    pub fn state() -> ServiceState {
        let Some(svc) = open(SERVICE_QUERY_STATUS | SERVICE_QUERY_CONFIG) else { return ServiceState::default() };
        let running = current_state(&svc) == Some(SERVICE_RUNNING);
        let command = command(&svc);
        let matches_this_exe = command.as_deref().is_some_and(|c| exe_of(c).eq_ignore_ascii_case(&this_exe()));
        let version = if running { ping().ok() } else { None };
        ServiceState { installed: true, running, command, matches_this_exe, version }
    }

    /// The SID of the user running this process.
    pub fn user_sid() -> Result<String, String> {
        unsafe {
            let mut token: HANDLE = null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return Err("读不到当前用户".into());
            }
            let mut need = 0u32;
            GetTokenInformation(token, TokenUser, null_mut(), 0, &mut need);
            let mut buf = vec![0u64; (need as usize).div_ceil(8).max(1)];
            let ok = GetTokenInformation(token, TokenUser, buf.as_mut_ptr() as *mut c_void, need, &mut need) != 0;
            CloseHandle(token);
            if !ok {
                return Err("读不到当前用户".into());
            }
            let user = &*(buf.as_ptr() as *const TOKEN_USER);
            let mut s: windows_sys::core::PWSTR = null_mut();
            if ConvertSidToStringSidW(user.User.Sid, &mut s) == 0 {
                return Err("读不到当前用户".into());
            }
            let sid = from_wide_ptr(s);
            LocalFree(s as *mut c_void);
            Ok(sid)
        }
    }

    fn start(svc: &Sc) -> Result<(), String> {
        if unsafe { StartServiceW(svc.0, 0, null()) } == 0 {
            let e = unsafe { GetLastError() };
            if e != ERROR_SERVICE_ALREADY_RUNNING {
                return Err(format!("启动后台服务失败（{e}）"));
            }
        }
        Ok(())
    }

    fn stop_and_wait(svc: &Sc) {
        let mut st: SERVICE_STATUS = unsafe { std::mem::zeroed() };
        unsafe { ControlService(svc.0, SERVICE_CONTROL_STOP, &mut st) };
        let end = Instant::now() + Duration::from_secs(10);
        while Instant::now() < end && current_state(svc).is_some_and(|s| s != SERVICE_STOPPED) {
            thread::sleep(Duration::from_millis(200));
        }
    }

    /// Installs (or points the existing service at this copy) and starts it. `sid`: the user
    /// the pipe lets in; None = the user running this process.
    pub fn install(sid: Option<String>) -> Result<ServiceState, String> {
        if !platform::is_admin() {
            return Err("需要管理员权限才能安装后台服务。".into());
        }
        let sid = match sid {
            Some(s) if valid_sid(&s) => s,
            Some(_) => return Err("用户 SID 格式不对".into()),
            None => user_sid()?,
        };
        let cmd = wide(&command_line(&this_exe(), &sid));
        if let Some(svc) = open(SERVICE_QUERY_STATUS | SERVICE_QUERY_CONFIG | SERVICE_START | SERVICE_STOP | SERVICE_ALL_ACCESS) {
            // Already there (maybe another copy): point it here, restart.
            stop_and_wait(&svc);
            let ok = unsafe {
                ChangeServiceConfigW(svc.0, SERVICE_NO_CHANGE, SERVICE_NO_CHANGE, SERVICE_NO_CHANGE, cmd.as_ptr(), null(), null_mut(), null(), null(), null(), null())
            };
            if ok == 0 {
                return Err(format!("修改后台服务失败（{}）", unsafe { GetLastError() }));
            }
            start(&svc)?;
        } else {
            let m = manager(SC_MANAGER_CONNECT | SC_MANAGER_CREATE_SERVICE)?;
            let (name, display) = (wide(SERVICE_NAME), wide("SplitGate 后台服务"));
            let h = unsafe {
                CreateServiceW(
                    m.0,
                    name.as_ptr(),
                    display.as_ptr(),
                    SERVICE_ALL_ACCESS,
                    SERVICE_WIN32_OWN_PROCESS,
                    SERVICE_AUTO_START,
                    SERVICE_ERROR_NORMAL,
                    cmd.as_ptr(),
                    null(),
                    null_mut(),
                    null(),
                    null(), // LocalSystem
                    null(),
                )
            };
            if h.is_null() {
                return Err(format!("安装后台服务失败（{}）", unsafe { GetLastError() }));
            }
            let svc = Sc(h);
            let mut text = wide("SplitGate 的后台部分：负责连接（虚拟网卡、sing-box、断线重连），SplitGate 本身就不用以管理员身份运行。移除：SplitGate 设置页“移除后台服务”。");
            let desc = SERVICE_DESCRIPTIONW { lpDescription: text.as_mut_ptr() };
            unsafe { ChangeServiceConfig2W(svc.0, SERVICE_CONFIG_DESCRIPTION, &desc as *const _ as *const c_void) };
            start(&svc)?;
        }
        // Give it a moment to open the pipe before the UI asks.
        let end = Instant::now() + Duration::from_secs(5);
        while Instant::now() < end && ping().is_err() {
            thread::sleep(Duration::from_millis(250));
        }
        Ok(state())
    }

    /// For an update (administrator): stop the service if it is there; true = it was running.
    pub fn stop_installed() -> Result<bool, String> {
        let Some(svc) = open(SERVICE_QUERY_STATUS | SERVICE_STOP) else { return Ok(false) };
        if current_state(&svc) != Some(SERVICE_RUNNING) {
            return Ok(false);
        }
        stop_and_wait(&svc);
        match current_state(&svc) {
            Some(SERVICE_STOPPED) => Ok(true),
            _ => Err("后台服务 10 秒内没有停下来".into()),
        }
    }

    /// Start the installed service again (after an update replaced its exe).
    pub fn start_installed() -> Result<(), String> {
        let Some(svc) = open(SERVICE_QUERY_STATUS | SERVICE_START) else { return Ok(()) };
        start(&svc)?;
        let end = Instant::now() + Duration::from_secs(5);
        while Instant::now() < end && ping().is_err() {
            thread::sleep(Duration::from_millis(250));
        }
        Ok(())
    }

    pub fn remove() -> Result<ServiceState, String> {
        if !platform::is_admin() {
            return Err("需要管理员权限才能移除后台服务。".into());
        }
        let Some(svc) = open(SERVICE_QUERY_STATUS | SERVICE_STOP | DELETE) else { return Ok(state()) };
        stop_and_wait(&svc);
        if unsafe { DeleteService(svc.0) } == 0 {
            return Err(format!("移除后台服务失败（{}）", unsafe { GetLastError() }));
        }
        drop(svc);
        Ok(state())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_reply_has_version() {
        let r = simple_reply(&parse_request(r#"{"cmd":"ping"}"#).unwrap()).unwrap();
        assert!(r.ok);
        assert_eq!(r.version.as_deref(), Some(env!("CARGO_PKG_VERSION")));
        assert!(!parse_request(r#"{"cmd":"format_disk"}"#).unwrap_err().ok);
        assert!(parse_request("not json").is_err());
    }

    #[test]
    fn protocol_shapes() {
        let c = parse_request(r#"{"cmd":"connect","profile_id":"school","main":"pw"}"#).unwrap();
        assert_eq!(c, Request::Connect { profile_id: "school".into(), main: "pw".into(), preshared_key: None });
        let ev = serde_json::to_value(Event::Status { status: crate::supervisor::Status::Disconnected, api: None }).unwrap();
        assert_eq!(ev["event"], "status");
        assert_eq!(ev["status"]["state"], "disconnected");
        // Replies have no "event" key: that is how the app tells them apart.
        assert!(serde_json::to_value(Reply::ok()).unwrap().get("event").is_none());
    }

    #[test]
    fn command_line_round_trip() {
        let c = command_line(r"D:\Apps\Split Gate\SplitGate.exe", "S-1-5-21-1-2-3-1001");
        assert_eq!(c, r#""D:\Apps\Split Gate\SplitGate.exe" --service --allow-sid S-1-5-21-1-2-3-1001"#);
        assert_eq!(exe_of(&c), r"D:\Apps\Split Gate\SplitGate.exe", "spaces in the path survive");
        assert_eq!(exe_of(r"C:\x\SplitGate.exe --service"), r"C:\x\SplitGate.exe");
    }

    #[test]
    fn only_real_sids_reach_the_sddl() {
        assert!(valid_sid("S-1-5-21-3623811015-3361044348-30300820-1013"));
        assert!(!valid_sid("S-1-5-21)(A;;GA;;;WD"), "no SDDL injection");
        assert!(!valid_sid("WD"));
        assert_eq!(pipe_sddl(Some("S-1-5-21-1-2-3-1001")), "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;S-1-5-21-1-2-3-1001)");
        assert_eq!(pipe_sddl(Some("S-1-1-0)(A;;GA;;;WD")), "D:P(A;;GA;;;SY)(A;;GA;;;BA)", "bad SID: admins only");
    }
}
