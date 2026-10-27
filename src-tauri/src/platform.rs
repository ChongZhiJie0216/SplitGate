//! Small Windows helpers.

use std::path::Path;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// CREATE_NO_WINDOW: child console programs must not flash a window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn is_admin() -> bool {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0
    }
    #[cfg(not(windows))]
    false
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct RunningProcess {
    pub name: String,
    pub path: String,
}

/// Running programs for the "pick a program" dialog: one entry per exe path, Windows' own
/// programs (under C:\Windows) left out. Programs whose path Windows will not tell us (system services,
/// other users) are left out too: they cannot be picked for a path rule.
#[cfg(windows)]
pub fn running_processes() -> Vec<RunningProcess> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};

    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into()).to_ascii_lowercase() + "\\";
    let mut out: Vec<RunningProcess> = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return out;
        }
        let mut e: PROCESSENTRY32W = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut e) != 0;
        while ok {
            let len = e.szExeFile.iter().position(|c| *c == 0).unwrap_or(e.szExeFile.len());
            let name = String::from_utf16_lossy(&e.szExeFile[..len]);
            let mut path = None;
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, e.th32ProcessID);
            if !h.is_null() {
                let mut buf = [0u16; 1024];
                let mut size = buf.len() as u32;
                if QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut size) != 0 {
                    path = Some(String::from_utf16_lossy(&buf[..size as usize]));
                }
                CloseHandle(h);
            }
            if let Some(path) = path {
                let system = path.to_ascii_lowercase().starts_with(&windir);
                if !system && !out.iter().any(|o| o.path.eq_ignore_ascii_case(&path)) {
                    out.push(RunningProcess { name, path });
                }
            }
            ok = Process32NextW(snap, &mut e) != 0;
        }
        CloseHandle(snap);
    }
    out.sort_by_key(|p| p.name.to_ascii_lowercase());
    out
}

#[cfg(not(windows))]
pub fn running_processes() -> Vec<RunningProcess> {
    Vec::new()
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn lists_itself_with_full_path() {
        let me = std::env::current_exe().unwrap().display().to_string();
        let list = super::running_processes();
        assert!(list.iter().any(|p| p.path.eq_ignore_ascii_case(&me)), "own exe not listed");
        assert!(!list.iter().any(|p| p.name.eq_ignore_ascii_case("csrss.exe")), "system processes should be hidden");
    }
}

/// Local time as (y, mo, d, h, mi, s) for log lines and file names.
pub fn local_time() -> (u16, u16, u16, u16, u16, u16) {
    #[cfg(windows)]
    unsafe {
        let mut t: windows_sys::Win32::Foundation::SYSTEMTIME = std::mem::zeroed();
        windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut t);
        (t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
    }
    #[cfg(not(windows))]
    (1970, 1, 1, 0, 0, 0)
}

/// A job object with KILL_ON_JOB_CLOSE: children put in it end when SplitGate ends,
/// even when SplitGate crashes or is killed in Task Manager (SPEC "错误处理与清理").
/// The handle is never closed on purpose: closing it is what kills the children.
#[cfg(windows)]
pub struct KillOnCloseJob(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
unsafe impl Send for KillOnCloseJob {}
#[cfg(windows)]
unsafe impl Sync for KillOnCloseJob {}

#[cfg(windows)]
impl KillOnCloseJob {
    pub fn new() -> Result<Self, String> {
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        unsafe {
            let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if h.is_null() {
                return Err("无法建立 Job Object".into());
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                h,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if ok == 0 {
                return Err("无法设置 Job Object".into());
            }
            Ok(Self(h))
        }
    }

    pub fn assign(&self, child: &std::process::Child) -> Result<(), String> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Foundation::GetLastError;
        use windows_sys::Win32::System::JobObjects::{AssignProcessToJobObject, IsProcessInJob};
        let h = child.as_raw_handle() as _;
        if unsafe { AssignProcessToJobObject(self.0, h) } == 0 {
            return Err(format!("无法把 sing-box 放进 Job Object（错误 {}）：SplitGate 被强行结束时它不会跟着结束", unsafe { GetLastError() }));
        }
        // Checked, not assumed: without it a killed SplitGate leaves sing-box and its routes behind.
        let mut inside = 0;
        if unsafe { IsProcessInJob(h, self.0, &mut inside) } == 0 || inside == 0 {
            return Err("sing-box 没有在 Job Object 里：SplitGate 被强行结束时它不会跟着结束".into());
        }
        Ok(())
    }
}

/// A Command for a console helper (sing-box etc.) that never opens a window.
pub fn hidden_command(exe: &Path) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(exe);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// IPv4 addresses of this PC as ("192.168.1.23/24", adapter name), without loopback, link-local
/// and our own SplitGate* adapters. Empty when it cannot be read.
pub fn local_ipv4_networks() -> Vec<(String, String)> {
    let Ok(out) = powershell(LOCAL_NETS_SCRIPT).output() else { return Vec::new() };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.trim().split_once('|').map(|(c, a)| (c.to_string(), a.to_string())))
        .collect()
}

/// IPv4 DNS servers of the adapters with a default route, best first, without SplitGate's TUN and
/// `skip` (the Pangolin client's adapter). Empty when they cannot be read.
pub fn physical_dns(skip: &str) -> Vec<String> {
    let Ok(out) = powershell(PHYSICAL_DNS_SCRIPT).env("SG_SKIP", skip).output() else { return Vec::new() };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| l.parse::<std::net::Ipv4Addr>().is_ok())
        .map(str::to_string)
        .collect()
}

const PHYSICAL_DNS_SCRIPT: &str = r#"
Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue |
    Where-Object { $_.InterfaceAlias -notlike 'SplitGate*' -and $_.InterfaceAlias -ne $env:SG_SKIP } |
    Sort-Object RouteMetric |
    ForEach-Object { (Get-DnsClientServerAddress -InterfaceIndex $_.ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue).ServerAddresses } |
    Select-Object -Unique
"#;

const LOCAL_NETS_SCRIPT: &str = r#"
Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
    Where-Object { $_.InterfaceAlias -notlike 'SplitGate*' -and $_.IPAddress -notlike '127.*' -and $_.IPAddress -notlike '169.254.*' } |
    ForEach-Object { "$($_.IPAddress)/$($_.PrefixLength)|$($_.InterfaceAlias)" }
"#;

/// Windows PowerShell 5.1 running `script` (pure ASCII) without profile or window.
/// Inputs go in through environment variables set by the caller, so nothing needs quoting.
pub fn powershell(script: &str) -> Command {
    let mut cmd = hidden_command(Path::new("powershell.exe"));
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &encode_command(script)]);
    cmd
}

/// powershell -EncodedCommand takes base64 of UTF-16LE.
fn encode_command(script: &str) -> String {
    let bytes: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    base64(&bytes)
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
const INSTANCE_MUTEX: &str = r"Local\SplitGate-instance";
#[cfg(windows)]
const SHOW_EVENT: &str = r"Local\SplitGate-show-window";

/// Only one copy may run: a second copy's startup cleanup would stop the first one's sing-box.
/// Returns false when another copy runs; it is then asked to show its window.
/// The mutex handle stays open until the process ends, which is what marks "running".
#[cfg(windows)]
pub fn claim_single_instance() -> bool {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::{CreateEventW, CreateMutexW, SetEvent};
    unsafe {
        let m = CreateMutexW(std::ptr::null(), 0, wide(INSTANCE_MUTEX).as_ptr());
        if !m.is_null() && GetLastError() != ERROR_ALREADY_EXISTS {
            return true;
        }
        let ev = CreateEventW(std::ptr::null(), 0, 0, wide(SHOW_EVENT).as_ptr());
        if !ev.is_null() {
            SetEvent(ev);
        }
        false
    }
}

#[cfg(not(windows))]
pub fn claim_single_instance() -> bool {
    true
}

/// Calls `show` each time a second copy is started.
#[cfg(windows)]
pub fn on_show_request(show: impl Fn() + Send + 'static) {
    use windows_sys::Win32::System::Threading::{CreateEventW, WaitForSingleObject, INFINITE};
    std::thread::spawn(move || unsafe {
        let ev = CreateEventW(std::ptr::null(), 0, 0, wide(SHOW_EVENT).as_ptr());
        if ev.is_null() {
            return;
        }
        while WaitForSingleObject(ev, INFINITE) == 0 {
            show();
        }
    });
}

#[cfg(not(windows))]
pub fn on_show_request(_show: impl Fn() + Send + 'static) {}

#[cfg(test)]
mod encode_tests {
    use super::*;

    #[test]
    fn local_nets_script_is_ascii() {
        assert!(LOCAL_NETS_SCRIPT.is_ascii());
    }

    #[test]
    fn base64_matches_known_values() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
