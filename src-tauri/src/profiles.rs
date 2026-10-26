//! Profile operations used by the commands: validate, keep secrets in step with profiles.
//! Works on an in-memory ProfilesFile so it can be tested without touching disk.

use serde::{Deserialize, Serialize};

use crate::config::{self, Profile, ProfilesFile};
use crate::credentials;

/// Secrets typed by the user. None / empty = keep what is stored.
#[derive(Deserialize, Default, Debug)]
pub struct SecretsInput {
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub preshared_key: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct ProfileView {
    pub profile: Profile,
    /// Cisco password / WireGuard private key is in the Credential Manager.
    pub has_secret: bool,
}

#[derive(Serialize, Debug)]
pub struct ProfilesView {
    pub active: Option<String>,
    pub profiles: Vec<ProfileView>,
}

pub fn view(file: &ProfilesFile) -> ProfilesView {
    ProfilesView {
        active: file.active.clone(),
        profiles: file
            .profiles
            .iter()
            .map(|p| ProfileView {
                profile: p.clone(),
                has_secret: credentials::main_account(p).is_none_or(|a| credentials::exists(&a)),
            })
            .collect(),
    }
}

fn non_empty(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// New id from the name's ASCII part, made unique ("p" if nothing usable).
pub fn new_id(file: &ProfilesFile, name: &str) -> String {
    let mut base: String = name
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    base.truncate(40);
    if base.is_empty() {
        base = "p".into();
    }
    let taken = |id: &str| file.profiles.iter().any(|p| p.id == id);
    if !taken(&base) {
        return base;
    }
    (2..).map(|n| format!("{base}-{n}")).find(|id| !taken(id)).unwrap()
}

/// Returns the id (new profiles get one here).
pub fn save(file: &mut ProfilesFile, mut profile: Profile, secrets: SecretsInput) -> Result<String, String> {
    if profile.id.is_empty() {
        profile.id = new_id(file, &profile.name);
    }
    // Keep only the sub-object that matches the type.
    match profile.kind {
        config::ExitType::Cisco => (profile.pangolin, profile.wireguard) = (None, None),
        config::ExitType::Pangolin => (profile.cisco, profile.wireguard) = (None, None),
        config::ExitType::Wireguard => (profile.cisco, profile.pangolin) = (None, None),
    }
    if let Some(c) = profile.cisco.as_mut() {
        c.server = c.server.trim().to_string();
        c.user = c.user.trim().to_string();
        c.auth_group = c.auth_group.take().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        c.bind_interface = c.bind_interface.take().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    }
    config::validate(&profile)?;

    let old = file.profiles.iter().find(|p| p.id == profile.id).cloned();
    let new_acc = credentials::main_account(&profile);
    let old_acc = old.as_ref().and_then(credentials::main_account);

    match (non_empty(&secrets.secret), &new_acc) {
        (Some(s), Some(acc)) => credentials::set(acc, s)?,
        (None, Some(acc)) => {
            // Cisco user renamed without retyping the password: move the stored one.
            let moved = match &old_acc {
                Some(o) if o != acc => credentials::get(o)?,
                _ => None,
            };
            if let Some(s) = moved {
                credentials::set(acc, &s)?;
            } else if !credentials::exists(acc) {
                return Err(match profile.kind {
                    config::ExitType::Wireguard => "请导入含 PrivateKey 的 .conf".into(),
                    _ => "请填写密码".into(),
                });
            }
        }
        _ => {}
    }
    if let Some(o) = &old_acc {
        if new_acc.as_ref() != Some(o) {
            credentials::delete(o)?;
        }
    }
    if let (Some(psk), Some(acc)) = (non_empty(&secrets.preshared_key), credentials::psk_account(&profile)) {
        credentials::set(&acc, psk)?;
    }

    match file.profiles.iter_mut().find(|p| p.id == profile.id) {
        Some(slot) => *slot = profile.clone(),
        None => file.profiles.push(profile.clone()),
    }
    if file.active.is_none() {
        file.active = Some(profile.id.clone());
    }
    Ok(profile.id)
}

pub fn delete(file: &mut ProfilesFile, id: &str) -> Result<(), String> {
    let pos = file.profiles.iter().position(|p| p.id == id).ok_or("找不到这个 Profile")?;
    let p = file.profiles.remove(pos);
    for acc in [credentials::main_account(&p), credentials::psk_account(&p)].into_iter().flatten() {
        credentials::delete(&acc)?;
    }
    if file.active.as_deref() == Some(id) {
        file.active = file.profiles.first().map(|p| p.id.clone());
    }
    Ok(())
}

pub fn set_active(file: &mut ProfilesFile, id: &str) -> Result<(), String> {
    if !file.profiles.iter().any(|p| p.id == id) {
        return Err("找不到这个 Profile".into());
    }
    file.active = Some(id.to_string());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::tests::cisco;
    use crate::credentials::{get, use_mock_store};

    fn pw(s: &str) -> SecretsInput {
        SecretsInput { secret: Some(s.into()), preshared_key: None }
    }

    // One test: the mock store is process-global, so the steps run in order.
    #[test]
    fn secrets_follow_profiles() {
        use_mock_store();
        let mut f = ProfilesFile::default();

        // New profile without a password is refused.
        assert!(save(&mut f, cisco("school"), SecretsInput::default()).is_err());
        save(&mut f, cisco("school"), pw("pw-1")).unwrap();
        assert_eq!(f.active.as_deref(), Some("school"));
        assert_eq!(get("school:user01").unwrap().as_deref(), Some("pw-1"));
        assert!(view(&f).profiles[0].has_secret);

        // Edit without retyping keeps the password.
        let mut p = cisco("school");
        p.name = "改名".into();
        save(&mut f, p.clone(), SecretsInput::default()).unwrap();
        assert_eq!(get("school:user01").unwrap().as_deref(), Some("pw-1"));

        // Changing the user moves the stored password to the new account key.
        p.cisco.as_mut().unwrap().user = "user02".into();
        save(&mut f, p, SecretsInput::default()).unwrap();
        assert_eq!(get("school:user01").unwrap(), None);
        assert_eq!(get("school:user02").unwrap().as_deref(), Some("pw-1"));

        // Deleting the profile deletes its secret and clears active.
        delete(&mut f, "school").unwrap();
        assert_eq!(get("school:user02").unwrap(), None);
        assert_eq!(f.active, None);
    }

    #[test]
    fn ids_are_slugs_and_unique() {
        let mut f = ProfilesFile::default();
        assert_eq!(new_id(&f, "学校 VPN"), "vpn");
        assert_eq!(new_id(&f, "家里"), "p");
        f.profiles.push(cisco("vpn"));
        assert_eq!(new_id(&f, "VPN!"), "vpn-2");
    }
}
