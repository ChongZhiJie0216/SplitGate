//! Exit adapters (SPEC 组件 0): one module per exit type. Each turns its Profile settings into
//! the sing-box VPN endpoint and the DNS server that resolves through it (`ExitAdapter`); the
//! Profile page's helpers live next to them (WireGuard .conf import, Cisco group list / test login).
//! Adding an exit (Pangolin, last in the project) = a new module + one arm in `adapter`.

use serde_json::Value;

use crate::config::{ExitType, Profile};
use crate::singbox::Secrets;

pub mod cisco;
pub mod wireguard;

/// What one exit adds to the sing-box config.
pub struct ExitParts {
    /// The endpoint, tagged `singbox::VPN_TAG`.
    pub endpoint: Value,
    /// The DNS server tagged "vpn-dns" that resolves through the exit.
    pub dns: Value,
    /// The VPN server's host (name or IP): always reached directly, never through the tunnel.
    pub server_host: String,
}

pub trait ExitAdapter {
    fn parts(&self, secrets: &Secrets) -> Result<ExitParts, String>;
}

/// The adapter for this profile's exit type.
pub fn adapter(profile: &Profile) -> Result<&dyn ExitAdapter, String> {
    match profile.kind {
        ExitType::Cisco => Ok(profile.cisco.as_ref().ok_or("缺少 Cisco 设置")?),
        ExitType::Wireguard => Ok(profile.wireguard.as_ref().ok_or("缺少 WireGuard 设置")?),
        ExitType::Pangolin => Err("Pangolin 出口还不能连接（排在整个项目最后才做）。".into()),
    }
}
