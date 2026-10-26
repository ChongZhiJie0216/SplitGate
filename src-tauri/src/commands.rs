//! Commands exposed to the frontend (types mirrored in src/lib/ipc.ts).

use serde::Serialize;

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
