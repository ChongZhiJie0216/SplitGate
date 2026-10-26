//! Commands exposed to the frontend (types mirrored in src/lib/ipc.ts). Thin wrappers only.

use serde::Serialize;

use crate::config::{self, Profile};
use crate::exits::{cisco, wireguard};
use crate::profiles::{self, ProfilesView, SecretsInput};
use crate::rules::{self, RuleSet, RuleSetView};
use crate::settings::{self, Settings};
use crate::{autostart, block_detect, credentials, diagnostics, logs, paths, platform, service, supervisor, traffic, update};

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

/// Load rules.json (adding missing presets), apply `change`, save when needed, return the view.
fn edit_rules(change: impl FnOnce(&mut rules::RulesFile) -> Result<bool, String>) -> Result<Vec<RuleSetView>, String> {
    let _guard = config::FILE_LOCK.lock().map_err(|_| "内部错误：文件锁")?;
    let (mut file, added) = rules::load()?;
    if change(&mut file)? || added {
        rules::save(&file)?;
    }
    Ok(rules::view(&file))
}

#[tauri::command]
pub fn list_rulesets() -> Result<Vec<RuleSetView>, String> {
    edit_rules(|_| Ok(false))
}

#[derive(Serialize)]
pub struct SaveRuleSetResult {
    id: String,
    rulesets: Vec<RuleSetView>,
}

#[tauri::command]
pub fn save_ruleset(ruleset: RuleSet) -> Result<SaveRuleSetResult, String> {
    let mut id = String::new();
    let rulesets = edit_rules(|f| {
        id = rules::upsert(f, ruleset)?;
        Ok(true)
    })?;
    Ok(SaveRuleSetResult { id, rulesets })
}

#[tauri::command]
pub fn reset_ruleset_to_preset(id: String) -> Result<Vec<RuleSetView>, String> {
    edit_rules(|f| rules::reset_to_preset(f, &id).map(|_| true))
}

/// Also removes the rule set from every profile, so both files change together.
#[tauri::command]
pub fn delete_ruleset(id: String) -> Result<Vec<RuleSetView>, String> {
    let _guard = config::FILE_LOCK.lock().map_err(|_| "内部错误：文件锁")?;
    let (mut file, _) = rules::load()?;
    let mut profiles = config::load_profiles()?;
    rules::delete(&mut file, &mut profiles, &id)?;
    rules::save(&file)?;
    config::save_profiles(&profiles)?;
    Ok(rules::view(&file))
}

#[tauri::command]
pub fn list_running_processes() -> Vec<platform::RunningProcess> {
    platform::running_processes()
}

#[tauri::command]
pub fn connect(profile_id: Option<String>) -> Result<(), String> {
    supervisor::connect(profile_id)
}

#[tauri::command]
pub fn disconnect() -> Result<(), String> {
    supervisor::disconnect()
}

#[tauri::command]
pub fn get_status() -> supervisor::Status {
    supervisor::status()
}

#[tauri::command]
pub fn get_logs() -> Vec<logs::LogLine> {
    logs::recent()
}

#[tauri::command]
pub fn import_wireguard_conf(text: String) -> Result<wireguard::ImportedConf, String> {
    wireguard::parse_conf(&text)
}

#[derive(Serialize)]
pub struct SettingsView {
    settings: Settings,
    autostart: autostart::AutostartState,
    /// settings.json could not be read: the page shows defaults and this message.
    error: Option<String>,
}

#[tauri::command]
pub fn get_settings() -> SettingsView {
    let (settings, error) = match settings::load() {
        Ok(s) => (s, None),
        Err(e) => (Settings::default(), Some(e)),
    };
    SettingsView { settings, autostart: autostart::state(), error }
}

#[tauri::command]
pub fn save_settings(settings: Settings) -> Result<Settings, String> {
    let _guard = config::FILE_LOCK.lock().map_err(|_| "内部错误：文件锁")?;
    settings::save(&settings)?;
    logs::info("设置已保存（下次连接时生效）");
    Ok(settings)
}

#[tauri::command]
pub fn set_autostart(enabled: bool) -> Result<autostart::AutostartState, String> {
    let r = autostart::set(enabled)?;
    logs::info(if r.enabled { "已开启开机自启（任务计划程序）" } else { "已关闭开机自启" });
    Ok(r)
}

#[tauri::command]
pub fn get_connections() -> traffic::Snapshot {
    traffic::snapshot()
}

#[tauri::command]
pub fn close_connection(id: String) -> Result<(), String> {
    traffic::close(&id)
}

#[tauri::command]
pub fn list_blocked() -> Vec<block_detect::Blocked> {
    block_detect::list()
}

#[tauri::command]
pub fn dismiss_blocked(host: String) -> Vec<block_detect::Blocked> {
    block_detect::dismiss(&host)
}

#[tauri::command]
pub fn clear_blocked() -> Vec<block_detect::Blocked> {
    block_detect::clear()
}

/// Returns the zip path. Runs PowerShell (network state, zip), so it takes a few seconds.
#[tauri::command]
pub async fn export_diagnostics(app: tauri::AppHandle) -> Result<String, String> {
    let version = app.package_info().version.to_string();
    tauri::async_runtime::spawn_blocking(move || diagnostics::export(&version, singbox_version()))
        .await
        .map_err(|e| e.to_string())?
        .map(|p| p.display().to_string())
}

#[tauri::command]
pub fn show_diagnostics(path: String) -> Result<(), String> {
    diagnostics::show(&path)
}

/// Asks GitHub for the latest release; takes a few seconds (PowerShell + two requests).
#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<update::UpdateInfo, String> {
    let version = app.package_info().version.to_string();
    tauri::async_runtime::spawn_blocking(move || update::check(&version)).await.map_err(|e| e.to_string())?
}

/// The CHANGELOG built into this exe, for the About page.
#[tauri::command]
pub fn get_changelog() -> Vec<update::Section> {
    update::own_changelog()
}

#[tauri::command]
pub fn open_release_page(url: String) -> Result<(), String> {
    update::open_release_page(&url)
}

/// Asks Windows and the service itself (pipe); a second or two at most.
#[tauri::command]
pub async fn get_service_state() -> Result<service::ServiceState, String> {
    tauri::async_runtime::spawn_blocking(service::state).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_service() -> Result<service::ServiceState, String> {
    let r = tauri::async_runtime::spawn_blocking(service::install).await.map_err(|e| e.to_string())?;
    if r.is_ok() {
        logs::info("已安装后台服务（SplitGateService）");
    }
    r
}

#[tauri::command]
pub async fn remove_service() -> Result<service::ServiceState, String> {
    let r = tauri::async_runtime::spawn_blocking(service::remove).await.map_err(|e| e.to_string())?;
    if r.is_ok() {
        logs::info("已移除后台服务");
    }
    r
}

/// The AnyLink / Cisco server's group list, for the Profile page's drop-down.
#[tauri::command]
pub async fn cisco_groups(server: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || cisco::groups(&server)).await.map_err(|e| e.to_string())?
}

/// Tests a login without building a tunnel. An empty password means "the saved one".
#[tauri::command]
pub async fn test_cisco_login(profile: Profile, password: Option<String>) -> Result<cisco::LoginResult, String> {
    let c = profile.cisco.clone().ok_or("缺少 Cisco 设置")?;
    let password = match password.filter(|p| !p.is_empty()) {
        Some(p) => p,
        None if !profile.id.is_empty() => credentials::main_account(&profile)
            .map(|a| credentials::get(&a))
            .transpose()?
            .flatten()
            .ok_or("还没有保存密码，请先填写密码")?,
        None => return Err("请先填写密码".into()),
    };
    logs::info(format!("测试 Cisco 登录：{}（账号 {}，组别 {}）", c.server, c.user, c.auth_group.as_deref().unwrap_or("无")));
    let r = tauri::async_runtime::spawn_blocking(move || cisco::test_login(&c.server, &c.user, c.auth_group.as_deref(), &password))
        .await
        .map_err(|e| e.to_string())?;
    logs::info(format!("测试 Cisco 登录结果：{r:?}"));
    Ok(r)
}
