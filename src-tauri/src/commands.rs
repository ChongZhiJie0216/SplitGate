//! Commands exposed to the frontend (types mirrored in src/lib/ipc.ts). Thin wrappers only.

use serde::Serialize;

use crate::config::{self, Profile};
use crate::exits::wireguard;
use crate::profiles::{self, ProfilesView, SecretsInput};
use crate::{paths, platform};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    singbox_version: Option<String>,
    is_admin: bool,
    data_dir: String,
}

#[tauri::command]
pub fn get_app_info(app: tauri::AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        singbox_version: singbox_version(),
        is_admin: platform::is_admin(),
        data_dir: paths::data_dir().display().to_string(),
    }
}

/// First line of `sing-box version`, e.g. "sing-box version 1.14.4".
fn singbox_version() -> Option<String> {
    let out = platform::hidden_command(&paths::singbox_exe()).arg("version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text.lines().next()?.trim();
    Some(first.trim_start_matches("sing-box version ").to_string())
}

/// Load profiles.json, apply `change`, save, return the new view.
fn edit_profiles(change: impl FnOnce(&mut config::ProfilesFile) -> Result<(), String>) -> Result<ProfilesView, String> {
    let _guard = config::FILE_LOCK.lock().map_err(|_| "内部错误：文件锁")?;
    let mut file = config::load_profiles()?;
    change(&mut file)?;
    config::save_profiles(&file)?;
    Ok(profiles::view(&file))
}

#[tauri::command]
pub fn list_profiles() -> Result<ProfilesView, String> {
    let _guard = config::FILE_LOCK.lock().map_err(|_| "内部错误：文件锁")?;
    Ok(profiles::view(&config::load_profiles()?))
}

#[derive(Serialize)]
pub struct SaveResult {
    id: String,
    view: ProfilesView,
}

#[tauri::command]
pub fn save_profile(profile: Profile, secrets: Option<SecretsInput>) -> Result<SaveResult, String> {
    let mut id = String::new();
    let view = edit_profiles(|f| {
        id = profiles::save(f, profile, secrets.unwrap_or_default())?;
        Ok(())
    })?;
    Ok(SaveResult { id, view })
}

#[tauri::command]
pub fn delete_profile(id: String) -> Result<ProfilesView, String> {
    edit_profiles(|f| profiles::delete(f, &id))
}

#[tauri::command]
pub fn set_active_profile(id: String) -> Result<ProfilesView, String> {
    edit_profiles(|f| profiles::set_active(f, &id))
}

#[tauri::command]
pub fn import_wireguard_conf(text: String) -> Result<wireguard::ImportedConf, String> {
    wireguard::parse_conf(&text)
}
