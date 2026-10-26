mod cleanup;
mod commands;
mod config;
mod credentials;
mod exits;
mod logs;
mod paths;
mod platform;
mod profiles;
mod rules;
mod singbox;
mod supervisor;

pub fn run() {
    // A failing Credential Manager must not stop the app: commands report it when used.
    let _ = credentials::init();
    let app = tauri::Builder::default()
        .setup(|app| {
            logs::init(app.handle().clone());
            supervisor::init(app.handle().clone());
            logs::info(format!("SplitGate v{} 启动，设置文件夹：{}", app.package_info().version, paths::data_dir().display()));
            // Leftovers from a crash or a forced exit (SPEC "错误处理与清理").
            std::thread::spawn(|| match cleanup::run(true) {
                Ok(r) if r.is_clean() => logs::info("启动检查：没有残留"),
                Ok(_) => {}
                Err(e) => logs::error(e),
            });
            Ok(())
        })
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
            commands::connect,
            commands::disconnect,
            commands::get_status,
            commands::get_logs,
        ])
        .build(tauri::generate_context!())
        .expect("error while building SplitGate");
    app.run(|_, event| {
        // Closing the window ends the app: disconnect first (sing-box restores the routes).
        if let tauri::RunEvent::Exit = event {
            supervisor::shutdown();
        }
    });
}
