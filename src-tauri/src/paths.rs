//! All file locations live here; other modules must not build paths themselves.
//! Settings: %APPDATA%\SplitGate (SPEC "数据模型"). SPLITGATE_HOME overrides it (dev and tests).

use std::path::PathBuf;

pub const APP_DIR_NAME: &str = "SplitGate";

pub fn data_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("SPLITGATE_HOME") {
        return PathBuf::from(home);
    }
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join(APP_DIR_NAME)
}

pub fn profiles_json() -> PathBuf {
    data_dir().join("profiles.json")
}

pub fn rules_json() -> PathBuf {
    data_dir().join("rules.json")
}

/// Folder of the running exe; Tauri puts the sing-box sidecar next to it.
pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn singbox_exe() -> PathBuf {
    exe_dir().join("sing-box.exe")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_override_wins() {
        std::env::set_var("SPLITGATE_HOME", r"C:\tmp\sg-test");
        assert_eq!(data_dir(), PathBuf::from(r"C:\tmp\sg-test"));
        std::env::remove_var("SPLITGATE_HOME");
        assert!(data_dir().ends_with(APP_DIR_NAME));
    }
}
