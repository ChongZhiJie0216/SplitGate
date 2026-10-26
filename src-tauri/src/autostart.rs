//! Start with Windows (settings page). The app needs administrator rights, and Windows skips
//! elevated programs in the Run registry key, so this uses a Task Scheduler task with "highest
//! privileges" that starts at the current user's logon. The task itself is the only record.

use std::path::Path;

use serde::Serialize;

use crate::platform;

pub const TASK_NAME: &str = "SplitGate";
/// Passed by the task: start hidden in the tray.
pub const AUTOSTART_ARG: &str = "--autostart";

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct AutostartState {
    pub enabled: bool,
    /// The program the task starts (None when there is no task).
    pub command: Option<String>,
    /// False when the task points at another copy (e.g. the portable folder was moved).
    pub matches_this_exe: bool,
}

pub fn state() -> AutostartState {
    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
    let out = platform::hidden_command(Path::new("schtasks.exe")).args(["/Query", "/TN", TASK_NAME, "/XML"]).output();
    let Ok(out) = out else { return AutostartState { enabled: false, command: None, matches_this_exe: false } };
    if !out.status.success() {
        return AutostartState { enabled: false, command: None, matches_this_exe: false };
    }
    let command = xml_tag(&decode(&out.stdout), "Command");
    let matches_this_exe = command.as_deref().is_some_and(|c| c.eq_ignore_ascii_case(&exe));
    AutostartState { enabled: true, command, matches_this_exe }
}

pub fn set(enabled: bool) -> Result<AutostartState, String> {
    if !platform::is_admin() {
        return Err("需要以管理员身份运行 SplitGate 才能修改开机自启。".into());
    }
    if enabled {
        let exe = std::env::current_exe().map_err(|e| format!("找不到程序位置：{e}"))?;
        let user = current_user();
        let xml = task_xml(&exe.display().to_string(), &user);
        // schtasks reads the XML file as UTF-16 with BOM.
        let path = std::env::temp_dir().join(format!("splitgate-task-{}.xml", std::process::id()));
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(xml.encode_utf16().flat_map(|u| u.to_le_bytes()));
        std::fs::write(&path, bytes).map_err(|e| format!("写入临时文件失败：{e}"))?;
        let r = run(&["/Create", "/TN", TASK_NAME, "/XML", &path.display().to_string(), "/F"]);
        let _ = std::fs::remove_file(&path);
        r?;
    } else if state().enabled {
        run(&["/Delete", "/TN", TASK_NAME, "/F"])?;
    }
    Ok(state())
}

fn run(args: &[&str]) -> Result<(), String> {
    let out = platform::hidden_command(Path::new("schtasks.exe"))
        .args(args)
        .output()
        .map_err(|e| format!("无法执行 schtasks：{e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let msg = decode(&out.stderr);
        Err(format!("任务计划程序拒绝了：{}", msg.trim()))
    }
}

fn current_user() -> String {
    let user = std::env::var("USERNAME").unwrap_or_default();
    match std::env::var("USERDOMAIN") {
        Ok(d) if !d.is_empty() => format!("{d}\\{user}"),
        _ => user,
    }
}

/// schtasks prints UTF-16 when redirected on some systems and the console code page on others.
fn decode(bytes: &[u8]) -> String {
    let utf16 = bytes.len() >= 2 && (bytes.starts_with(&[0xFF, 0xFE]) || bytes[1] == 0);
    if utf16 {
        let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        String::from_utf16_lossy(&units).trim_start_matches('\u{feff}').to_string()
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

fn xml_tag(xml: &str, tag: &str) -> Option<String> {
    let start = xml.find(&format!("<{tag}>"))? + tag.len() + 2;
    let end = start + xml[start..].find(&format!("</{tag}>"))?;
    Some(unescape(xml[start..end].trim()))
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn unescape(s: &str) -> String {
    s.replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

/// Defaults changed on purpose: no 72-hour time limit, and it also starts on battery (laptop).
fn task_xml(exe: &str, user: &str) -> String {
    let (exe, user) = (escape(exe), escape(user));
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>SplitGate: start in the tray at logon (Settings page)</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
      <Delay>PT10S</Delay>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>{AUTOSTART_ARG}</Arguments>
    </Exec>
  </Actions>
</Task>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_xml_round_trip() {
        let xml = task_xml(r"C:\Apps\A&B\SplitGate.exe", r"PC\测试甲");
        assert_eq!(xml_tag(&xml, "Command").as_deref(), Some(r"C:\Apps\A&B\SplitGate.exe"));
        assert_eq!(xml_tag(&xml, "Arguments").as_deref(), Some(AUTOSTART_ARG));
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"), "never stopped after 72 hours");
        assert!(xml.contains("<RunLevel>HighestAvailable</RunLevel>"));
        assert!(xml.contains(r"<UserId>PC\测试甲</UserId>"));
    }

    #[test]
    fn decodes_utf16_and_utf8() {
        let s = "<Command>C:\\x.exe</Command>";
        let mut utf16 = vec![0xFF, 0xFE];
        utf16.extend(s.encode_utf16().flat_map(|u| u.to_le_bytes()));
        assert_eq!(decode(&utf16), s);
        assert_eq!(decode(s.as_bytes()), s);
    }
}
