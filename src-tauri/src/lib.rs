mod autostart;
mod backend;
mod block_detect;
mod cleanup;
mod commands;
mod config;
mod credentials;
mod diagnostics;
mod elevate;
mod exits;
mod logs;
mod paths;
mod platform;
mod profiles;
mod remote;
mod rules;
mod service;
mod settings;
mod singbox;
mod subscription;
mod supervisor;
mod traffic;
mod tray;
mod update;
mod updater;

use tauri::Manager;

pub fn run() {
    // `SplitGate.exe --service`: the background service (no window, no tray, no single-instance check).
    if service::is_service_launch() {
        service::run_service();
        return;
    }
    // `--admin-task`: one administrator-only step for a copy without those rights; exits here.
    elevate::run_task_from_args();
    // `--wait-pid`: "restart as administrator"; the old copy must be gone before the check below.
    elevate::wait_for_previous();
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
            // Up to v0.26 the home LAN networks were a Profile field; now they are a rule set.
            let home_moved = config::FILE_LOCK.lock().map_err(|_| "内部错误：文件锁".to_string()).and_then(|_g| rules::migrate_files());
            logs::init(handle.clone());
            supervisor::init(handle.clone());
            remote::init(handle.clone());
            logs::info(format!("SplitGate v{} 启动，设置文件夹：{}", app.package_info().version, paths::data_dir().display()));
            match home_moved {
                Ok(true) => logs::info("已把 Profile 里的家里内网网段移到规则集“家里内网”，并在原来的 Profile 里勾选"),
                Ok(false) => {}
                Err(e) => logs::error(format!("移动家里内网网段失败：{e}")),
            }
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
            // Rule set subscriptions: due ones are updated a minute after start, then every 6 hours.
            subscription::start_auto_update(handle.clone());
            // Files the last update replaced (SplitGate.exe.old …) and its download.
            updater::clean_leftovers();
            // Leftovers from a crash or a forced exit (SPEC "错误处理与清理"), then the optional auto-connect.
            std::thread::spawn(|| {
                match cleanup::run(true) {
                    Ok(r) if r.is_clean() => logs::info("启动检查：没有残留"),
                    Ok(_) => {}
                    Err(e) => logs::error(e),
                }
                if settings::load_or_default().auto_connect {
                    if !platform::is_admin() && !backend::can_connect_without_admin() {
                        logs::warn("设置了启动后自动连接，但没有以管理员身份运行，也没有可用的后台服务，跳过。");
                    } else if let Err(e) = backend::connect(None) {
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
            commands::cisco_groups,
            commands::test_cisco_login,
            commands::list_rulesets,
            commands::save_ruleset,
            commands::delete_ruleset,
            commands::update_subscription,
            commands::list_running_processes,
            commands::connect,
            commands::disconnect,
            commands::get_status,
            commands::get_logs,
            commands::get_settings,
            commands::save_settings,
            commands::set_autostart,
            commands::get_connections,
            commands::get_vpn_usage,
            commands::reset_vpn_usage,
            commands::close_connection,
            commands::list_blocked,
            commands::dismiss_blocked,
            commands::clear_blocked,
            commands::export_diagnostics,
            commands::show_diagnostics,
            commands::check_update,
            commands::get_changelog,
            commands::get_licenses,
            commands::get_service_state,
            commands::install_service,
            commands::remove_service,
            commands::restart_as_admin,
            commands::open_release_page,
            commands::download_update,
            commands::get_update_job,
            commands::install_update,
        ])
        .build(tauri::generate_context!())
        .expect("error while building SplitGate");
    app.run(|_, event| {
        // Quitting (tray menu "退出") ends the app: disconnect first (sing-box restores the routes).
        if let tauri::RunEvent::Exit = event {
            backend::shutdown();
        }
    });
}
