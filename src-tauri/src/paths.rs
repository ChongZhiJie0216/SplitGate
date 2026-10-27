//! All file locations live here; other modules must not build paths themselves.
//! Settings, logs and diagnostics: the `data` folder next to SplitGate.exe (maintainer's choice
//! 2026-10-26, v0.12.0; before that %APPDATA%\SplitGate). SPLITGATE_HOME overrides it (dev and tests).

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

pub const APP_DIR_NAME: &str = "SplitGate";
pub const DATA_DIR_NAME: &str = "data";
/// Settings files copied from the old location; logs and diagnostics are not.
const MIGRATED_FILES: [&str; 3] = ["profiles.json", "rules.json", "settings.json"];

/// The background service runs its own copy from a protected folder (v0.20.0) but works for one
/// SplitGate copy: its `--app` argument sets that copy's folder (the settings are there).
static APP_DIR: OnceLock<PathBuf> = OnceLock::new();

// Only the Windows background service calls this.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn set_app_dir(dir: PathBuf) {
    let _ = APP_DIR.set(dir);
}

/// Folder of the SplitGate copy whose settings are used: this exe's, or for the service the copy
/// it was installed for.
pub fn app_dir() -> PathBuf {
    APP_DIR.get().cloned().unwrap_or_else(exe_dir)
}

pub fn data_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("SPLITGATE_HOME") {
        return PathBuf::from(home);
    }
    app_dir().join(DATA_DIR_NAME)
}

/// Where the background service's own copy of SplitGate.exe and sing-box.exe lives (v0.20.0):
/// `%ProgramFiles%\SplitGate`, which only administrators can change. The service runs as SYSTEM,
/// so it must not run files any program of the user could replace.
pub fn service_dir() -> PathBuf {
    let pf = std::env::var_os("ProgramW6432").or_else(|| std::env::var_os("ProgramFiles")).map(PathBuf::from);
    pf.unwrap_or_else(|| PathBuf::from(r"C:\Program Files")).join(APP_DIR_NAME)
}

/// Where settings lived up to v0.11: %APPDATA%\SplitGate.
fn old_data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(APP_DIR_NAME))
}

/// Run once at startup, before anything reads a settings file: copies each settings file the new
/// folder does not have yet from the old one. The old folder stays as it is. Returns what was copied.
pub fn migrate_old_data() -> Vec<String> {
    if std::env::var_os("SPLITGATE_HOME").is_some() {
        return Vec::new();
    }
    let Some(old) = old_data_dir() else { return Vec::new() };
    copy_missing(&old, &data_dir())
}

fn copy_missing(from: &std::path::Path, to: &std::path::Path) -> Vec<String> {
    let mut done = Vec::new();
    for name in MIGRATED_FILES {
        let (src, dst) = (from.join(name), to.join(name));
        if src.is_file() && !dst.exists() && fs::create_dir_all(to).is_ok() && fs::copy(&src, &dst).is_ok() {
            done.push(format!("{} → {}", src.display(), dst.display()));
        }
    }
    done
}

pub fn profiles_json() -> PathBuf {
    data_dir().join("profiles.json")
}

pub fn rules_json() -> PathBuf {
    data_dir().join("rules.json")
}

pub fn settings_json() -> PathBuf {
    data_dir().join("settings.json")
}

pub fn logs_dir() -> PathBuf {
    data_dir().join("logs")
}

pub fn diagnostics_dir() -> PathBuf {
    data_dir().join("diagnostics")
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

/// Every sing-box.exe SplitGate may have started: this copy's and the service's (cleanup).
pub fn our_singbox_exes() -> Vec<PathBuf> {
    let mut v = vec![singbox_exe(), app_dir().join("sing-box.exe"), service_dir().join("sing-box.exe")];
    v.dedup();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_override_wins() {
        std::env::set_var("SPLITGATE_HOME", r"C:\tmp\sg-test");
        assert_eq!(data_dir(), PathBuf::from(r"C:\tmp\sg-test"));
        std::env::remove_var("SPLITGATE_HOME");
        assert_eq!(data_dir(), exe_dir().join("data"), "next to the exe");
    }

    #[test]
    fn migration_copies_only_missing_settings() {
        let base = std::env::temp_dir().join(format!("sg-migrate-{}", std::process::id()));
        let (old, new) = (base.join("old"), base.join("new"));
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&new).unwrap();
        fs::write(old.join("profiles.json"), "old-profiles").unwrap();
        fs::write(old.join("rules.json"), "old-rules").unwrap();
        fs::write(new.join("rules.json"), "new-rules").unwrap();
        fs::write(old.join("splitgate-2026-10-26.log"), "x").unwrap();
        let done = copy_missing(&old, &new);
        assert_eq!(done.len(), 1, "{done:?}");
        assert_eq!(fs::read_to_string(new.join("profiles.json")).unwrap(), "old-profiles");
        assert_eq!(fs::read_to_string(new.join("rules.json")).unwrap(), "new-rules", "never overwrite the new folder");
        assert!(!new.join("splitgate-2026-10-26.log").exists(), "logs are not copied");
        assert!(old.join("profiles.json").exists(), "old folder left as it is");
        fs::remove_dir_all(&base).unwrap();
    }
}
