//! Who handles connect / disconnect: the background service when it can be used (remote.rs),
//! otherwise the app itself (supervisor.rs, needs administrator rights). Commands, the tray,
//! auto-connect and app exit all go through here.

use crate::supervisor::{self, Status};
use crate::{logs, remote, service};

pub fn status() -> Status {
    remote::status().unwrap_or_else(supervisor::status)
}

pub fn connect(profile_id: Option<String>) -> Result<(), String> {
    // Never let the service start a second sing-box while the app runs one itself.
    let local_busy = !matches!(supervisor::status(), Status::Disconnected | Status::Error { .. });
    if !local_busy {
        match remote::try_connect(profile_id.clone()) {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(e) => {
                logs::error(format!("经后台服务连接失败：{e}"));
                return Err(e);
            }
        }
    }
    supervisor::connect(profile_id)
}

pub fn disconnect() -> Result<(), String> {
    if remote::active() {
        remote::disconnect()
    } else if remote::status().is_some() {
        remote::clear();
        Ok(())
    } else {
        supervisor::disconnect()
    }
}

/// App closing.
pub fn shutdown() {
    remote::shutdown();
    supervisor::shutdown();
}

/// Connecting is possible without administrator rights when the service will do it.
pub fn can_connect_without_admin() -> bool {
    service::usable()
}
