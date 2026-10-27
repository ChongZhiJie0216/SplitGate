//! Secrets in the Windows Credential Manager (SPEC "凭据"): service "SplitGate",
//! account "<profile_id>:<user>" for Cisco, "<profile_id>:wireguard" / ":wireguard-psk" for WireGuard,
//! "<profile_id>:pangolin-proxy" for the Pangolin exit's home proxy (v0.21.0).
//! Secrets are never logged or returned to the frontend.

use keyring_core::{Entry, Error};

use crate::config::{ExitType, Profile};

pub const SERVICE: &str = "SplitGate";

/// Install the native store once at startup (tests install the mock store instead).
pub fn init() -> Result<(), String> {
    #[cfg(windows)]
    {
        let store = windows_native_keyring_store::Store::new().map_err(|e| format!("无法打开 Windows 凭据管理器：{e}"))?;
        keyring_core::set_default_store(store);
    }
    Ok(())
}

/// Account key of the main secret (Cisco password, WireGuard private key or Pangolin proxy password).
pub fn main_account(p: &Profile) -> Option<String> {
    match p.kind {
        ExitType::Cisco => p.cisco.as_ref().map(|c| format!("{}:{}", p.id, c.user.trim())),
        ExitType::Wireguard => Some(format!("{}:wireguard", p.id)),
        ExitType::Pangolin => Some(format!("{}:pangolin-proxy", p.id)),
    }
}

/// What to tell the user when the main secret is not stored yet.
pub fn missing_message(kind: ExitType) -> &'static str {
    match kind {
        ExitType::Wireguard => "还没有保存 WireGuard 私钥。请到“Profile”页重新导入 .conf 并保存。",
        ExitType::Pangolin => "还没有保存家里代理的密码。请到“Profile”页填写代理密码并保存。",
        ExitType::Cisco => "还没有保存密码。请到“Profile”页填写密码并保存。",
    }
}

pub fn psk_account(p: &Profile) -> Option<String> {
    (p.kind == ExitType::Wireguard).then(|| format!("{}:wireguard-psk", p.id))
}

fn entry(account: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, account).map_err(|e| format!("凭据管理器出错：{e}"))
}

pub fn set(account: &str, secret: &str) -> Result<(), String> {
    entry(account)?.set_password(secret).map_err(|e| format!("保存到凭据管理器失败：{e}"))
}

pub fn get(account: &str) -> Result<Option<String>, String> {
    match entry(account)?.get_password() {
        Ok(s) => Ok(Some(s)),
        Err(Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("读取凭据失败：{e}")),
    }
}

pub fn delete(account: &str) -> Result<(), String> {
    match entry(account)?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("删除凭据失败：{e}")),
    }
}

pub fn exists(account: &str) -> bool {
    matches!(get(account), Ok(Some(_)))
}

#[cfg(test)]
pub(crate) fn use_mock_store() {
    keyring_core::set_default_store(keyring_core::mock::Store::new().unwrap());
}

#[cfg(test)]
mod tests {
    /// Touches the real Windows Credential Manager, so it only runs on request:
    /// cargo test -- --ignored native_store
    #[test]
    #[ignore]
    fn native_store_roundtrip() {
        super::init().unwrap();
        let acc = format!("selftest-{}:user", std::process::id());
        super::set(&acc, "synthetic-secret").unwrap();
        assert_eq!(super::get(&acc).unwrap().as_deref(), Some("synthetic-secret"));
        super::delete(&acc).unwrap();
        assert_eq!(super::get(&acc).unwrap(), None);
    }
}
