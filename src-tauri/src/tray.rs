//! System tray (SPEC "交互要点"): the icon's corner dot shows the status; the menu has connect,
//! disconnect, open window and quit. Closing the window only hides it, the connection stays.

use std::sync::OnceLock;

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::backend;
use crate::supervisor::Status;
use crate::logs;

struct Tray {
    icon: TrayIcon,
    connect: MenuItem<Wry>,
    disconnect: MenuItem<Wry>,
    base: Image<'static>,
}

static TRAY: OnceLock<Tray> = OnceLock::new();

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let connect = MenuItem::with_id(app, "connect", "连接", true, None::<&str>)?;
    let disconnect = MenuItem::with_id(app, "disconnect", "断开", false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "打开主窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出（会先断开）", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&connect, &disconnect, &sep, &open, &quit])?;
    let base = app.default_window_icon().cloned().map(|i| i.to_owned()).unwrap_or_else(|| Image::new_owned(vec![0; 4], 1, 1));
    let status = backend::status();
    let icon = TrayIconBuilder::with_id("main")
        .icon(with_dot(&base, &status))
        .tooltip(tooltip(&status))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            // Off the main thread: through the background service these wait for its answer.
            "connect" => {
                let app = app.clone();
                std::thread::spawn(move || {
                    if let Err(e) = backend::connect(None) {
                        logs::error(&e);
                        show_window(&app);
                    }
                });
            }
            "disconnect" => {
                std::thread::spawn(|| {
                    if let Err(e) = backend::disconnect() {
                        logs::error(e);
                    }
                });
            }
            "open" => show_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_window(tray.app_handle());
            }
        })
        .build(app)?;
    let _ = TRAY.set(Tray { icon, connect, disconnect, base });
    update(app, &status);
    Ok(())
}

pub fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Called on every status change, from any thread; the tray is changed on the main thread.
pub fn update(app: &AppHandle, status: &Status) {
    let status = status.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(t) = TRAY.get() else { return };
        let idle = matches!(status, Status::Disconnected | Status::Error { retrying: false, .. });
        let _ = t.connect.set_enabled(idle);
        let _ = t.disconnect.set_enabled(!idle && status != Status::Disconnecting);
        let _ = t.icon.set_icon(Some(with_dot(&t.base, &status)));
        let _ = t.icon.set_tooltip(Some(tooltip(&status)));
    });
}

fn tooltip(status: &Status) -> String {
    match status {
        Status::Disconnected => "SplitGate：已断开".into(),
        Status::Connecting { profile_name, .. } => format!("SplitGate：连接中（{profile_name}）"),
        Status::Connected { profile_name, .. } => format!("SplitGate：已连接（{profile_name}）"),
        Status::Disconnecting => "SplitGate：断开中".into(),
        Status::Error { retrying: true, .. } => "SplitGate：连接中断，等待自动重新连接".into(),
        Status::Error { .. } => "SplitGate：错误，打开窗口看详情".into(),
    }
}

/// Grey = disconnected, amber = busy (also waiting to reconnect), green = connected, red = error.
fn dot_color(status: &Status) -> [u8; 3] {
    match status {
        Status::Disconnected => [0x9a, 0xa0, 0xa6],
        Status::Connecting { .. } | Status::Disconnecting | Status::Error { retrying: true, .. } => [0xf2, 0xa9, 0x00],
        Status::Connected { .. } => [0x1e, 0xa4, 0x4f],
        Status::Error { .. } => [0xd9, 0x30, 0x25],
    }
}

/// The app icon with a status dot (white ring) in the bottom-right corner.
fn with_dot(base: &Image<'_>, status: &Status) -> Image<'static> {
    let (w, h) = (base.width(), base.height());
    let mut rgba = base.rgba().to_vec();
    let size = w.min(h) as f32;
    let r = size * 0.24;
    let ring = (size * 0.05).max(1.0);
    let (cx, cy) = (w as f32 - r - ring, h as f32 - r - ring);
    let [cr, cg, cb] = dot_color(status);
    for y in 0..h {
        for x in 0..w {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            let px = match d {
                d if d <= r => [cr, cg, cb, 255],
                d if d <= r + ring => [255, 255, 255, 255],
                _ => continue,
            };
            let i = ((y * w + x) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&px);
        }
    }
    Image::new_owned(rgba, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_drawn_in_corner_only() {
        let base = Image::new_owned(vec![0; 32 * 32 * 4], 32, 32);
        let img = with_dot(&base, &Status::Disconnected);
        let px = |x: u32, y: u32| img.rgba()[((y * 32 + x) * 4) as usize..][..4].to_vec();
        assert_eq!(px(0, 0), vec![0, 0, 0, 0], "top-left untouched");
        assert_eq!(px(24, 24), vec![0x9a, 0xa0, 0xa6, 255], "grey dot when disconnected");
        let img = with_dot(&base, &Status::Error { message: String::new(), retrying: false });
        assert_eq!(img.rgba()[((24 * 32 + 24) * 4) as usize], 0xd9);
    }
}
