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

/// A Command for a console helper (sing-box etc.) that never opens a window.
pub fn hidden_command(exe: &Path) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(exe);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}
