//! One administrator-only step from an app that runs without administrator rights (v0.18.0).
//! The same exe is started elevated (Windows asks with a UAC prompt) with `--admin-task`, does only
//! that step, writes the outcome to a small file and exits; nothing else of the app starts there.
//! Also "restart as administrator" (`--wait-pid`: the new copy waits until this one has gone, or
//! it would only bring this window to the front).
// Only Windows starts elevated copies; elsewhere the argument handling is only used by the tests.
#![cfg_attr(not(windows), allow(dead_code))]

use std::path::{Path, PathBuf};

pub const ADMIN_TASK_ARG: &str = "--admin-task";
pub const WAIT_PID_ARG: &str = "--wait-pid";
const RESULT_ARG: &str = "--result";

#[derive(Debug, Clone, PartialEq)]
pub enum Task {
    /// `sid`: the user the service's pipe lets in (the one running the app, not the admin who
    /// answered the prompt, which can be another account).
    InstallService { sid: String },
    RemoveService,
    /// `least`: the task starts SplitGate without administrator rights (the service connects).
    AutostartOn { user: String, least: bool },
    AutostartOff,
    /// Install a downloaded version (updater.rs): stop the service, replace the files, start it.
    ApplyUpdate { src: String },
}

impl Task {
    fn args(&self) -> Vec<String> {
        match self {
            Task::InstallService { sid } => vec!["install-service".into(), sid.clone()],
            Task::RemoveService => vec!["remove-service".into()],
            Task::AutostartOn { user, least } => vec!["autostart-on".into(), if *least { "least" } else { "highest" }.into(), user.clone()],
            Task::AutostartOff => vec!["autostart-off".into()],
            Task::ApplyUpdate { src } => vec!["apply-update".into(), src.clone()],
        }
    }
}

/// `--admin-task <task...> --result <file>` from a command line (without the program name).
fn parse(args: &[String]) -> Option<(Task, PathBuf)> {
    let i = args.iter().position(|a| a == ADMIN_TASK_ARG)?;
    let rest = &args[i + 1..];
    let result = rest.iter().position(|a| a == RESULT_ARG).and_then(|j| rest.get(j + 1)).map(PathBuf::from)?;
    // A value is never another option (a missing value must not swallow "--result").
    let value = |k: usize| rest.get(k).filter(|v| !v.starts_with("--"));
    let task = match (rest.first()?.as_str(), value(1), value(2)) {
        ("install-service", Some(sid), _) => Task::InstallService { sid: sid.clone() },
        ("remove-service", _, _) => Task::RemoveService,
        ("autostart-on", Some(level), Some(user)) if level == "least" || level == "highest" => {
            Task::AutostartOn { user: user.clone(), least: level == "least" }
        }
        ("autostart-off", _, _) => Task::AutostartOff,
        ("apply-update", Some(src), _) => Task::ApplyUpdate { src: src.clone() },
        _ => return None,
    };
    Some((task, result))
}

/// One argument quoted the way Windows splits command lines (CommandLineToArgvW rules).
fn quote(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_string();
    }
    let mut out = String::from('"');
    let mut slashes = 0;
    for c in arg.chars() {
        match c {
            '\\' => slashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(slashes * 2 + 1));
                out.push('"');
                slashes = 0;
                continue;
            }
            _ => {}
        }
        if c != '\\' {
            out.push_str(&"\\".repeat(slashes));
            slashes = 0;
            out.push(c);
        }
    }
    out.push_str(&"\\".repeat(slashes * 2));
    out.push('"');
    out
}

fn command_line(task: &Task, result: &Path) -> String {
    let mut parts = vec![ADMIN_TASK_ARG.to_string()];
    parts.extend(task.args());
    parts.push(RESULT_ARG.into());
    parts.push(result.display().to_string());
    parts.iter().map(|p| quote(p)).collect::<Vec<_>>().join(" ")
}

/// In the elevated copy: run the task, leave the outcome, exit. Returns when not such a launch.
pub fn run_task_from_args() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((task, result)) = parse(&args) else {
        if args.iter().any(|a| a == ADMIN_TASK_ARG) {
            std::process::exit(2);
        }
        return;
    };
    let outcome = match task {
        Task::InstallService { sid } => crate::service::install(Some(sid)).map(|_| ()),
        Task::RemoveService => crate::service::remove().map(|_| ()),
        Task::AutostartOn { user, least } => crate::autostart::set_as(true, &user, least).map(|_| ()),
        Task::AutostartOff => crate::autostart::set_as(false, "", false).map(|_| ()),
        Task::ApplyUpdate { src } => crate::updater::apply_with_service(Path::new(&src)),
    };
    let _ = std::fs::write(&result, outcome.as_ref().err().map(String::as_str).unwrap_or("ok"));
    std::process::exit(if outcome.is_ok() { 0 } else { 1 });
}

/// In the app: run `task` in an elevated copy and wait for it (the user may take a while to
/// answer the prompt). Err carries the copy's own message, or why it did not run.
#[cfg(windows)]
pub fn run(task: &Task) -> Result<(), String> {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_CANCELLED, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};

    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let result = std::env::temp_dir().join(format!("splitgate-admin-{}-{n}.txt", std::process::id()));
    let _ = std::fs::remove_file(&result);
    let process = shell_execute_admin(&command_line(task, &result), true).map_err(|e| {
        if e == ERROR_CANCELLED {
            "已取消：没有同意 Windows 的管理员权限提示。".to_string()
        } else {
            format!("无法以管理员身份执行（{e}）")
        }
    })?;
    let (waited, code) = unsafe {
        let waited = WaitForSingleObject(process, 10 * 60 * 1000) == WAIT_OBJECT_0;
        let mut code = 1u32;
        if waited && GetExitCodeProcess(process, &mut code) == 0 {
            code = GetLastError();
        }
        CloseHandle(process);
        (waited, code)
    };
    let text = std::fs::read_to_string(&result).unwrap_or_default();
    let _ = std::fs::remove_file(&result);
    match (waited, code) {
        (false, _) => Err("以管理员身份执行的步骤 10 分钟没有结束".into()),
        (true, 0) => Ok(()),
        _ if !text.is_empty() => Err(text),
        (true, code) => Err(format!("以管理员身份执行的步骤失败（代码 {code}）")),
    }
}

/// Start this exe again as administrator; the caller then quits so the new copy can take over.
#[cfg(windows)]
pub fn relaunch_as_admin() -> Result<(), String> {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_CANCELLED};
    let args = format!("{WAIT_PID_ARG} {}", std::process::id());
    match shell_execute_admin(&args, false) {
        Ok(h) => {
            if !h.is_null() {
                unsafe { CloseHandle(h) };
            }
            Ok(())
        }
        Err(e) if e == ERROR_CANCELLED => Err("已取消：没有同意 Windows 的管理员权限提示。".into()),
        Err(e) => Err(format!("无法以管理员身份重新启动（{e}）")),
    }
}

#[cfg(not(windows))]
pub fn run(_task: &Task) -> Result<(), String> {
    Err("只支持 Windows".into())
}

#[cfg(not(windows))]
pub fn relaunch_as_admin() -> Result<(), String> {
    Err("只支持 Windows".into())
}

/// ShellExecuteEx with the "runas" verb on this exe. Returns the process handle when asked.
#[cfg(windows)]
fn shell_execute_admin(params: &str, want_process: bool) -> Result<windows_sys::Win32::Foundation::HANDLE, u32> {
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let exe = std::env::current_exe().map_err(|_| 2u32)?;
    let (verb, file, params) = (wide("runas"), wide(&exe.display().to_string()), wide(params));
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOASYNC | if want_process { SEE_MASK_NOCLOSEPROCESS } else { 0 };
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params.as_ptr();
    info.nShow = 1; // SW_SHOWNORMAL: the restarted app shows its window; an admin task has none
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        return Err(unsafe { GetLastError() });
    }
    Ok(info.hProcess)
}

/// `--wait-pid N`: started by "restart as administrator"; wait (at most 15 s) for the old copy.
pub fn wait_for_previous() {
    let args: Vec<String> = std::env::args().collect();
    let Some(pid) = args.iter().position(|a| a == WAIT_PID_ARG).and_then(|i| args.get(i + 1)).and_then(|p| p.parse::<u32>().ok()) else {
        return;
    };
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};
        let h = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if !h.is_null() {
            WaitForSingleObject(h, 15_000);
            CloseHandle(h);
        }
    }
    #[cfg(not(windows))]
    let _ = pid;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Splits like CommandLineToArgvW does for arguments (not the program name).
    fn split(line: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        let (mut quoted, mut any, mut slashes) = (false, false, 0usize);
        for c in line.chars() {
            match c {
                '\\' => {
                    slashes += 1;
                    continue;
                }
                '"' => {
                    cur.push_str(&"\\".repeat(slashes / 2));
                    if slashes % 2 == 1 {
                        cur.push('"');
                    } else {
                        quoted = !quoted;
                        any = true;
                    }
                }
                ' ' if !quoted => {
                    cur.push_str(&"\\".repeat(slashes));
                    if !cur.is_empty() || any {
                        out.push(std::mem::take(&mut cur));
                    }
                    any = false;
                }
                _ => {
                    cur.push_str(&"\\".repeat(slashes));
                    cur.push(c);
                }
            }
            slashes = 0;
        }
        cur.push_str(&"\\".repeat(slashes));
        if !cur.is_empty() || any {
            out.push(cur);
        }
        out
    }

    #[test]
    fn tasks_round_trip_through_the_command_line() {
        let result = PathBuf::from(r"C:\Users\测试 甲\AppData\Local\Temp\splitgate-admin-1-1.txt");
        for task in [
            Task::InstallService { sid: "S-1-5-21-1-2-3-1001".into() },
            Task::RemoveService,
            Task::AutostartOn { user: r"PC\测试 甲".into(), least: true },
            Task::AutostartOn { user: r"DOMAIN\a\".into(), least: false },
            Task::AutostartOff,
            Task::ApplyUpdate { src: r"D:\Apps\Split Gate\data\update\0.19.0\SplitGate".into() },
        ] {
            let line = command_line(&task, &result);
            assert_eq!(parse(&split(&line)), Some((task, result.clone())), "{line}");
        }
    }

    #[test]
    fn quoting_follows_windows_rules() {
        assert_eq!(quote("plain"), "plain");
        assert_eq!(quote("a b"), "\"a b\"");
        assert_eq!(quote(r"C:\dir\ x\"), r#""C:\dir\ x\\""#);
        assert_eq!(quote(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(quote(""), "\"\"");
    }

    #[test]
    fn unknown_or_incomplete_tasks_are_refused() {
        let v = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(parse(&v(&["--admin-task", "format-disk", "--result", "x"])), None);
        assert_eq!(parse(&v(&["--admin-task", "install-service", "--result", "x"])), None);
        assert_eq!(parse(&v(&["--admin-task", "remove-service"])), None);
        assert_eq!(parse(&v(&["--autostart"])), None);
    }
}
