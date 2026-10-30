//! rules.json: rule sets (SPEC "数据模型"). Since v0.27.0 there are no built-in presets: a new
//! rules.json starts empty, and the Xbox / Minecraft / Steam sets older versions added are ordinary ones.

use std::fs;

use serde::{Deserialize, Serialize};

use crate::config::{self, ProfilesFile};
use crate::paths;
use crate::subscription::{self, Subscription};

pub const RULES_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RuleSet {
    pub id: String,
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub domain_suffix: Vec<String>,
    #[serde(default)]
    pub domain_keyword: Vec<String>,
    /// Full paths: preferred over names, which many programs share (javaw.exe), phase 0 V6.
    #[serde(default)]
    pub process_path: Vec<String>,
    #[serde(default)]
    pub process_name: Vec<String>,
    #[serde(default)]
    pub ip_cidr: Vec<String>,
    /// Downloaded list (v0.26.0); its domains live in the two fields below, apart from the typed ones.
    #[serde(default)]
    pub subscription: Option<Subscription>,
    #[serde(default)]
    pub sub_domain_suffix: Vec<String>,
    #[serde(default)]
    pub sub_domain_keyword: Vec<String>,
}

#[cfg(test)]
pub(crate) fn empty_for_tests(id: &str) -> RuleSet {
    RuleSet {
        id: id.into(),
        name: "测试".into(),
        enabled: true,
        domain_suffix: vec![],
        domain_keyword: vec![],
        process_path: vec![],
        process_name: vec![],
        ip_cidr: vec![],
        subscription: None,
        sub_domain_suffix: vec![],
        sub_domain_keyword: vec![],
    }
}

fn yes() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RulesFile {
    pub version: u32,
    #[serde(default)]
    pub rulesets: Vec<RuleSet>,
}

impl Default for RulesFile {
    fn default() -> Self {
        Self { version: RULES_VERSION, rulesets: Vec::new() }
    }
}

#[derive(Serialize, Debug)]
pub struct RuleSetView {
    pub ruleset: RuleSet,
}

pub fn view(file: &RulesFile) -> Vec<RuleSetView> {
    file.rulesets.iter().map(|r| RuleSetView { ruleset: r.clone() }).collect()
}

/// Missing file = no rule sets.
pub fn load() -> Result<RulesFile, String> {
    let path = paths::rules_json();
    match fs::read_to_string(&path) {
        Ok(t) => serde_json::from_str::<RulesFile>(t.trim_start_matches('\u{feff}'))
            .map_err(|e| format!("{} 格式错误（第 {} 行）：{e}。请修正或删除这个文件。", path.display(), e.line())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(RulesFile::default()),
        Err(e) => Err(format!("读取 {} 失败：{e}", path.display())),
    }
}

pub fn save(file: &RulesFile) -> Result<(), String> {
    config::atomic_write_json(&paths::rules_json(), file)
}

/// Clean up user input (trim, lower-case domains, drop duplicates) and check every entry.
pub fn normalize(r: &mut RuleSet) -> Result<(), String> {
    r.name = r.name.trim().to_string();
    if r.name.is_empty() {
        return Err("请填写规则集名称".into());
    }
    if !config::is_slug(&r.id) {
        return Err("规则集内部编号无效".into());
    }
    r.domain_suffix = clean(&r.domain_suffix, |s| {
        let d = s.to_ascii_lowercase();
        let d = d.trim_start_matches("*.").trim_start_matches('.').to_string();
        valid_domain(&d).then_some(d).ok_or_else(|| format!("域名“{s}”格式不对（例：xboxlive.com）"))
    })?;
    r.domain_keyword = clean(&r.domain_keyword, |s| {
        let k = s.to_ascii_lowercase();
        (!k.contains(char::is_whitespace) && !k.contains('/')).then_some(k).ok_or_else(|| format!("关键字“{s}”不能有空格或 /"))
    })?;
    r.process_path = clean(&r.process_path, |s| {
        let ok = s.len() > 3 && s.as_bytes()[1] == b':' && s.to_ascii_lowercase().ends_with(".exe");
        ok.then(|| s.to_string()).ok_or_else(|| format!("程序路径“{s}”要写完整路径（例：C:\\Games\\app.exe）"))
    })?;
    r.process_name = clean(&r.process_name, |s| {
        let ok = s.to_ascii_lowercase().ends_with(".exe") && !s.contains(['\\', '/']);
        ok.then(|| s.to_string()).ok_or_else(|| format!("程序名“{s}”要写成 xxx.exe，不含文件夹"))
    })?;
    r.ip_cidr = clean(&r.ip_cidr, |s| {
        let c = if s.contains('/') {
            s.to_string()
        } else if s.contains(':') {
            format!("{s}/128")
        } else {
            format!("{s}/32")
        };
        config::check_cidr(&c).map(|_| c)
    })?;
    if let Some(s) = r.subscription.as_mut() {
        s.url = s.url.trim().to_string();
        subscription::resolve_url(&s.url)?;
    }
    Ok(())
}

fn clean(items: &[String], f: impl Fn(&str) -> Result<String, String>) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for s in items.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let v = f(s)?;
        if !out.iter().any(|o| o.eq_ignore_ascii_case(&v)) {
            out.push(v);
        }
    }
    Ok(out)
}

pub(crate) fn valid_domain(d: &str) -> bool {
    d.len() <= 253
        && d.contains('.')
        && d.split('.').all(|l| {
            !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-') && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}

fn new_id(file: &RulesFile) -> String {
    (1..).map(|n| format!("custom-{n}")).find(|id| !file.rulesets.iter().any(|r| &r.id == id)).unwrap()
}

/// Returns the id (new rule sets get one here).
pub fn upsert(file: &mut RulesFile, mut r: RuleSet) -> Result<String, String> {
    if r.id.is_empty() {
        r.id = new_id(file);
    }
    normalize(&mut r)?;
    let old = file.rulesets.iter().find(|x| x.id == r.id).cloned();
    subscription::keep_or_clear(&mut r, old.as_ref());
    let id = r.id.clone();
    match file.rulesets.iter_mut().find(|x| x.id == r.id) {
        Some(slot) => *slot = r,
        None => file.rulesets.push(r),
    }
    Ok(id)
}

/// Also drops the id from every profile.
pub fn delete(file: &mut RulesFile, profiles: &mut ProfilesFile, id: &str) -> Result<(), String> {
    let pos = file.rulesets.iter().position(|r| r.id == id).ok_or("找不到这个规则集")?;
    file.rulesets.remove(pos);
    for p in &mut profiles.profiles {
        p.ruleset_ids.retain(|x| x != id);
    }
    Ok(())
}

/// Up to v0.26 a profile had its own "home LAN networks" field. Each different list becomes a
/// rule set named 家里内网 (ip_cidr only), ticked in the profiles that had it; the field is cleared.
/// Returns whether anything changed.
pub fn migrate_home_cidrs(file: &mut RulesFile, profiles: &mut ProfilesFile) -> bool {
    let mut changed = false;
    for p in profiles.profiles.iter_mut().filter(|p| !p.home_cidrs.is_empty()) {
        let cidrs = std::mem::take(&mut p.home_cidrs);
        let same = |r: &&RuleSet| r.id.starts_with(HOME_ID) && r.ip_cidr.len() == cidrs.len() && cidrs.iter().all(|c| r.ip_cidr.contains(c));
        let id = match file.rulesets.iter().find(same) {
            Some(r) => r.id.clone(),
            None => {
                let n = file.rulesets.iter().filter(|r| r.id.starts_with(HOME_ID)).count();
                let id = (1..).map(|k| if k == 1 { HOME_ID.to_string() } else { format!("{HOME_ID}-{k}") }).find(|id| !file.rulesets.iter().any(|r| &r.id == id)).unwrap();
                let mut r = RuleSet {
                    id: id.clone(),
                    name: if n == 0 { "家里内网".into() } else { format!("家里内网（{}）", p.name) },
                    enabled: true,
                    domain_suffix: vec![],
                    domain_keyword: vec![],
                    process_path: vec![],
                    process_name: vec![],
                    ip_cidr: cidrs.clone(),
                    subscription: None,
                    sub_domain_suffix: vec![],
                    sub_domain_keyword: vec![],
                };
                // The old field only took valid networks; keep them as they were if one is not.
                let _ = normalize(&mut r);
                file.rulesets.push(r);
                id
            }
        };
        if !p.ruleset_ids.contains(&id) {
            p.ruleset_ids.push(id);
        }
        changed = true;
    }
    changed
}

const HOME_ID: &str = "home-lan";

/// Runs [`migrate_home_cidrs`] on the saved files. The caller holds `config::FILE_LOCK`.
pub fn migrate_files() -> Result<bool, String> {
    let mut profiles = config::load_profiles()?;
    if profiles.profiles.iter().all(|p| p.home_cidrs.is_empty()) {
        return Ok(false);
    }
    let mut file = load()?;
    migrate_home_cidrs(&mut file, &mut profiles);
    // Rule set first: if the second write fails, the old field is still there and the next start retries.
    save(&file)?;
    config::save_profiles(&profiles)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty(id: &str) -> RuleSet {
        empty_for_tests(id)
    }

    #[test]
    fn normalize_cleans_and_rejects() {
        let mut r = empty("a");
        r.domain_suffix = vec![" *.Xbox.com ".into(), "xbox.com".into(), ".steamstatic.com".into(), "".into()];
        r.ip_cidr = vec!["10.0.0.1".into()];
        r.process_path = vec![r"C:\Games\Prism\javaw.exe".into()];
        normalize(&mut r).unwrap();
        assert_eq!(r.domain_suffix, vec!["xbox.com", "steamstatic.com"]);
        assert_eq!(r.ip_cidr, vec!["10.0.0.1/32"]);

        for bad in ["xbox", "a b.com", "http://x.com"] {
            let mut r = empty("a");
            r.domain_suffix = vec![bad.into()];
            assert!(normalize(&mut r).is_err(), "{bad} should be rejected");
        }
        let mut r = empty("a");
        r.process_path = vec!["javaw.exe".into()];
        assert!(normalize(&mut r).is_err(), "a bare name is not a path");
    }

    #[test]
    fn delete_drops_the_id_from_profiles() {
        let mut f = RulesFile::default();
        let mut profiles = ProfilesFile::default();
        let a = upsert(&mut f, empty("")).unwrap();
        let b = upsert(&mut f, empty("")).unwrap();
        assert_eq!((a.as_str(), b.as_str()), ("custom-1", "custom-2"));
        let mut p = crate::config::tests::cisco("school");
        p.ruleset_ids = vec![a.clone(), b.clone()];
        profiles.profiles.push(p);
        delete(&mut f, &mut profiles, &a).unwrap();
        assert_eq!(profiles.profiles[0].ruleset_ids, vec![b]);
        assert!(delete(&mut f, &mut profiles, "nope").is_err());
    }

    #[test]
    fn home_cidrs_become_a_rule_set() {
        let mut f = RulesFile::default();
        let mut profiles = ProfilesFile::default();
        for (id, home) in [("a", vec!["192.168.1.0/24", "10.8.0.0/16"]), ("b", vec!["10.8.0.0/16", "192.168.1.0/24"]), ("c", vec!["192.168.77.0/24"]), ("d", vec![])] {
            let mut p = crate::config::tests::cisco(id);
            p.name = id.to_uppercase();
            p.ruleset_ids = vec!["steam".into()];
            p.home_cidrs = home.into_iter().map(String::from).collect();
            profiles.profiles.push(p);
        }
        assert!(migrate_home_cidrs(&mut f, &mut profiles));
        assert_eq!(f.rulesets.len(), 2, "same networks share one rule set");
        assert_eq!((f.rulesets[0].id.as_str(), f.rulesets[0].name.as_str()), ("home-lan", "家里内网"));
        assert_eq!(f.rulesets[0].ip_cidr, vec!["192.168.1.0/24", "10.8.0.0/16"]);
        assert_eq!((f.rulesets[1].id.as_str(), f.rulesets[1].name.as_str()), ("home-lan-2", "家里内网（C）"));
        let ids: Vec<_> = profiles.profiles.iter().map(|p| p.ruleset_ids.join(",")).collect();
        assert_eq!(ids, vec!["steam,home-lan", "steam,home-lan", "steam,home-lan-2", "steam"]);
        assert!(profiles.profiles.iter().all(|p| p.home_cidrs.is_empty()));
        assert!(!migrate_home_cidrs(&mut f, &mut profiles), "runs once");
    }
}
