mod commands;
mod paths;
mod platform;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::get_app_info])
        .run(tauri::generate_context!())
        .expect("error while running SplitGate");
}
