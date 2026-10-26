mod commands;
mod config;
mod credentials;
mod exits;
mod paths;
mod platform;
mod profiles;
mod rules;

pub fn run() {
    // A failing Credential Manager must not stop the app: commands report it when used.
    let _ = credentials::init();
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::list_profiles,
            commands::save_profile,
            commands::delete_profile,
            commands::set_active_profile,
            commands::import_wireguard_conf,
            commands::list_rulesets,
            commands::save_ruleset,
            commands::delete_ruleset,
            commands::reset_ruleset_to_preset,
            commands::list_running_processes,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SplitGate");
}
