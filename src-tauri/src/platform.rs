//! Small Windows helpers.

use std::path::Path;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// CREATE_NO_WINDOW: child console programs must not flash a window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn is_admin() -> bool {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0
    }
    #[cfg(not(windows))]
    false
}

/// A Command for a console helper (sing-box etc.) that never opens a window.
pub fn hidden_command(exe: &Path) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(exe);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}
