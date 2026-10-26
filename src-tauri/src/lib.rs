mod autostart;
mod block_detect;
mod cleanup;
mod commands;
mod config;
mod credentials;
mod diagnostics;
mod exits;
mod logs;
mod paths;
mod platform;
mod profiles;
mod rules;
mod settings;
mod singbox;
mod supervisor;
mod traffic;
mod tray;

use tauri::Manager;

pub fn run() {
    if !platform::claim_single_instance() {
        return; // the running copy shows its window instead
    }
    // A failing Credential Manager must not stop the app: commands report it when used.
    let _ = credentials::init();
    let autostarted = std::env::args().any(|a| a == autostart::AUTOSTART_ARG);
    let app = tauri::Builder::default()
        .setup(move |app| {
            let handle = app.handle().clone();
            // Before logs::init creates the new folder and before any settings file is read.
            let migrated = paths::migrate_old_data();
            logs::init(handle.clone());
            supervisor::init(handle.clone());
            logs::info(format!("SplitGate v{} 启动，设置文件夹：{}", app.package_info().version, paths::data_dir().display()));
            for m in migrated {
                logs::info(format!("已从旧的设置文件夹复制：{m}（旧文件夹没有改动，确认无误后可以自己删掉）"));
            }
            let tray_ok = match tray::init(&handle) {
                Ok(()) => true,
                Err(e) => {
                    logs::error(format!("建立托盘图标失败：{e}"));
                    false
                }
            };
            // Started at logon: stay in the tray. Without a tray the window must show, or it could not be reached.
            if !(autostarted && tray_ok) {
                tray::show_window(&handle);
            }
            let h = handle.clone();
            platform::on_show_request(move || tray::show_window(&h));
            // Leftovers from a crash or a forced exit (SPEC "错误处理与清理"), then the optional auto-connect.
            std::thread::spawn(|| {
                match cleanup::run(true) {
                    Ok(r) if r.is_clean() => logs::info("启动检查：没有残留"),
                    Ok(_) => {}
                    Err(e) => logs::error(e),
                }
                if settings::load_or_default().auto_connect {
                    if !platform::is_admin() {
                        logs::warn("设置了启动后自动连接，但没有以管理员身份运行，跳过。");
                    } else if let Err(e) = supervisor::connect(None) {
                        logs::error(format!("启动后自动连接失败：{e}"));
                    }
                }
            });
            if tray_ok {
                // Closing the window hides it in the tray; the connection stays (SPEC "交互要点").
                if let Some(w) = app.get_webview_window("main") {
                    let w2 = w.clone();
                    w.on_window_event(move |e| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                            api.prevent_close();
                            let _ = w2.hide();
                        }
                    });
                }
            }
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
            commands::get_settings,
            commands::save_settings,
            commands::set_autostart,
            commands::get_connections,
            commands::close_connection,
            commands::list_blocked,
            commands::dismiss_blocked,
            commands::clear_blocked,
            commands::export_diagnostics,
            commands::show_diagnostics,
        ])
        .build(tauri::generate_context!())
        .expect("error while building SplitGate");
    app.run(|_, event| {
        // Quitting (tray menu "退出") ends the app: disconnect first (sing-box restores the routes).
        if let tauri::RunEvent::Exit = event {
            supervisor::shutdown();
        }
    });
}
