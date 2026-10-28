//! rules.json: rule sets (SPEC "数据模型"). Built-in presets are compiled in; they are copied into
//! rules.json the first time they are missing, can be edited and disabled, but not deleted.

use std::fs;

use serde::{Deserialize, Serialize};

use crate::config::{self, ProfilesFile};
use crate::paths;
use crate::subscription::{self, Subscription};

pub const RULES_VERSION: u32 = 1;

const PRESETS: [&str; 3] = [
    include_str!("../resources/presets/xbox.json"),
    include_str!("../resources/presets/minecraft.json"),
    include_str!("../resources/presets/steam.json"),
];

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
    pub preset: bool,
}

pub fn presets() -> Vec<RuleSet> {
    PRESETS.iter().map(|t| serde_json::from_str(t).expect("built-in preset is valid JSON")).collect()
}

pub fn preset(id: &str) -> Option<RuleSet> {
    presets().into_iter().find(|p| p.id == id)
}

pub fn view(file: &RulesFile) -> Vec<RuleSetView> {
    file.rulesets
        .iter()
        .map(|r| RuleSetView { ruleset: r.clone(), preset: preset(&r.id).is_some() })
        .collect()
}

/// Missing file = presets only. Missing presets are added back (they cannot be deleted).
/// Returns whether something was added, so the caller can save.
pub fn load() -> Result<(RulesFile, bool), String> {
    let path = paths::rules_json();
    let mut file = match fs::read_to_string(&path) {
        Ok(t) => serde_json::from_str::<RulesFile>(t.trim_start_matches('\u{feff}'))
            .map_err(|e| format!("{} 格式错误（第 {} 行）：{e}。请修正或删除这个文件。", path.display(), e.line()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => RulesFile::default(),
        Err(e) => return Err(format!("读取 {} 失败：{e}", path.display())),
    };
    let added = add_missing_presets(&mut file);
    Ok((file, added))
}

pub fn add_missing_presets(file: &mut RulesFile) -> bool {
    let mut added = false;
    for p in presets() {
        if !file.rulesets.iter().any(|r| r.id == p.id) {
            file.rulesets.push(p);
            added = true;
        }
    }
    added
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

/// Built-in presets can only be disabled or reset. Also drops the id from every profile.
pub fn delete(file: &mut RulesFile, profiles: &mut ProfilesFile, id: &str) -> Result<(), String> {
    if preset(id).is_some() {
        return Err("内置预设不能删除，可以关掉或恢复预设".into());
    }
    let pos = file.rulesets.iter().position(|r| r.id == id).ok_or("找不到这个规则集")?;
    file.rulesets.remove(pos);
    for p in &mut profiles.profiles {
        p.ruleset_ids.retain(|x| x != id);
    }
    Ok(())
}

pub fn reset_to_preset(file: &mut RulesFile, id: &str) -> Result<(), String> {
    let p = preset(id).ok_or("这个规则集不是内置预设")?;
    match file.rulesets.iter_mut().find(|r| r.id == id) {
        Some(slot) => *slot = p,
        None => file.rulesets.push(p),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty(id: &str) -> RuleSet {
        empty_for_tests(id)
    }

    /// Domains confirmed in phase 0 V5 (2026-10-26): these unblocked PrismLauncher and Steam logins.
    #[test]
    fn presets_match_phase0_findings() {
        let all: Vec<String> = presets().into_iter().flat_map(|p| p.domain_suffix).collect();
        for d in ["xboxlive.com", "xbox.com", "minecraft.net", "steampowered.com", "steamcommunity.com", "steamserver.net", "steamstatic.com", "steamcdn-a.akamaihd.net"] {
            assert!(all.iter().any(|x| x == d), "preset missing {d}");
        }
        assert!(!all.iter().any(|x| x == "akamaihd.net"), "never route the whole akamaihd.net");
        for mut p in presets() {
            normalize(&mut p).unwrap();
        }
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
    fn presets_come_back_and_cannot_be_deleted() {
        let mut f = RulesFile::default();
        assert!(add_missing_presets(&mut f));
        assert!(!add_missing_presets(&mut f));
        let mut profiles = ProfilesFile::default();
        assert!(delete(&mut f, &mut profiles, "xbox").is_err());

        let id = upsert(&mut f, empty("")).unwrap();
        assert_eq!(id, "custom-1");
        let mut p = crate::config::tests::cisco("school");
        p.ruleset_ids = vec!["xbox".into(), id.clone()];
        profiles.profiles.push(p);
        delete(&mut f, &mut profiles, &id).unwrap();
        assert_eq!(profiles.profiles[0].ruleset_ids, vec!["xbox"]);

        f.rulesets[0].domain_suffix.clear();
        reset_to_preset(&mut f, "xbox").unwrap();
        assert_eq!(f.rulesets[0], preset("xbox").unwrap());
    }
}
