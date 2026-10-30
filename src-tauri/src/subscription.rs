//! Rule set subscriptions (v0.26.0, maintainer's choice 2026-10-28: SplitGate downloads text lists
//! itself, any https address, automatic update; how often is set per subscription, default 7 days). A subscribed rule set keeps the
//! downloaded domains apart from the ones typed by hand (`sub_domain_suffix` / `sub_domain_keyword`),
//! so an update never touches the user's own entries. Only domain rules: a list can never add
//! programs or IP ranges. A failed download keeps the previous list.
//!
//! Formats, recognised line by line (so one parser reads them all):
//! - v2fly domain-list-community: `spotify.com`, `full:x`, `domain:x`, `keyword:x`, `regexp:` (skipped),
//!   `include:name` (another file next to this one), attributes `@ads` (skipped when asked);
//! - plain lists: one domain per line, `*.x`, `.x`, `+.x`;
//! - Clash rules: `DOMAIN-SUFFIX,x`, `DOMAIN,x`, `DOMAIN-KEYWORD,x` (others skipped), also in a
//!   YAML `payload:` list.
//!
//! `full:` / `DOMAIN,` (one exact host) become suffix rules: they then also cover that host's
//! subdomains, which is close enough and keeps one kind of rule.

use std::collections::HashSet;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::rules::{self, RuleSet, RuleSetView};
use crate::{config, logs, platform};

/// v2fly's lists, for a bare name like `spotify`.
const V2FLY_BASE: &str = "https://raw.githubusercontent.com/v2fly/domain-list-community/master/data/";
/// Most domains one subscription may bring (a huge list would send almost everything to the VPN).
pub const MAX_ENTRIES: usize = 20_000;
/// Most files one subscription may read through `include:`.
const MAX_FILES: usize = 30;
/// Default update interval: 7 days.
const DEFAULT_INTERVAL_HOURS: u32 = 7 * 24;
/// How often the app looks for due subscriptions (the shortest interval is 1 hour).
const CHECK_EVERY: Duration = Duration::from_secs(10 * 60);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Subscription {
    /// https address, or a bare v2fly list name (`spotify`).
    pub url: String,
    /// Leave out entries marked `@ads` (v2fly).
    #[serde(default = "yes")]
    pub skip_ads: bool,
    /// Update automatically after this many hours; 0 = only by hand (v0.26.0).
    #[serde(default = "default_interval")]
    pub interval_hours: u32,
    /// Last successful update (ms since 1970); 0 = never.
    #[serde(default)]
    pub updated_ms: u64,
    /// Domains from the last successful update.
    #[serde(default)]
    pub count: usize,
    /// Lines that were not usable (regexp, IP or program rules, bad names).
    #[serde(default)]
    pub skipped: usize,
    /// Why the last update failed; None after a successful one.
    #[serde(default)]
    pub error: Option<String>,
}

fn yes() -> bool {
    true
}

fn default_interval() -> u32 {
    DEFAULT_INTERVAL_HOURS
}

impl Default for Subscription {
    fn default() -> Self {
        Subscription { url: String::new(), skip_ads: true, interval_hours: DEFAULT_INTERVAL_HOURS, updated_ms: 0, count: 0, skipped: 0, error: None }
    }
}

/// The address to download: a bare name means v2fly's list of that name.
pub fn resolve_url(input: &str) -> Result<String, String> {
    let s = input.trim();
    if !s.contains("://") {
        let ok = !s.is_empty() && s.len() < 100 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '!' | '@'));
        return if ok { Ok(format!("{V2FLY_BASE}{s}")) } else { Err(format!("订阅地址“{s}”要写 https:// 开头的网址，或 v2fly 列表的名字（例：spotify）")) };
    }
    if !s.starts_with("https://") || s.contains(char::is_whitespace) || s.contains('"') {
        return Err(format!("订阅地址“{s}”只接受 https:// 开头的网址"));
    }
    Ok(s.to_string())
}

#[derive(Debug, Default, PartialEq)]
pub struct Parsed {
    pub suffix: Vec<String>,
    pub keyword: Vec<String>,
    /// Names from `include:` lines.
    pub includes: Vec<String>,
    pub skipped: usize,
}

/// One downloaded file. Pure: tested without network.
pub fn parse(text: &str, skip_ads: bool) -> Parsed {
    let mut p = Parsed::default();
    for raw in text.lines() {
        // Comments: `#` everywhere, `//` in some plain lists.
        let line = raw.split('#').next().unwrap_or("");
        let line = line.split(" //").next().unwrap_or("").trim();
        if line.is_empty() || line == "payload:" || line.starts_with("//") {
            continue;
        }
        // YAML list item, quoted.
        let line = line.trim_start_matches("- ").trim().trim_matches(|c| c == '\'' || c == '"');
        // v2fly attributes after a space: `x.com @ads @cn`.
        let mut parts = line.split_whitespace();
        let Some(entry) = parts.next() else { continue };
        if skip_ads && parts.any(|a| a == "@ads") {
            continue;
        }
        let lower = entry.to_ascii_lowercase();
        let (kind, value) = if let Some(v) = lower.strip_prefix("include:") {
            p.includes.push(v.to_string());
            continue;
        } else if let Some(v) = lower.strip_prefix("keyword:").or_else(|| lower.strip_prefix("domain-keyword,")) {
            ("keyword", v.to_string())
        } else if let Some(v) = lower
            .strip_prefix("full:")
            .or_else(|| lower.strip_prefix("domain:"))
            .or_else(|| lower.strip_prefix("domain-suffix,"))
            .or_else(|| lower.strip_prefix("domain,"))
        {
            ("suffix", v.to_string())
        } else if lower.starts_with("regexp:") || lower.contains(',') || lower.contains(':') {
            // regexp, IP-CIDR, PROCESS-NAME, …: never from a subscription.
            p.skipped += 1;
            continue;
        } else {
            ("suffix", lower)
        };
        // Clash rules may carry a policy after another comma: `DOMAIN-SUFFIX,x.com,Proxy`.
        let value = value.split(',').next().unwrap_or("").trim();
        let value = value.trim_start_matches("+.").trim_start_matches("*.").trim_start_matches('.');
        let ok = match kind {
            "keyword" => !value.is_empty() && !value.contains(['/', ' ']),
            _ => rules::valid_domain(value),
        };
        match (ok, kind) {
            (false, _) => p.skipped += 1,
            (true, "keyword") => p.keyword.push(value.to_string()),
            (true, _) => p.suffix.push(value.to_string()),
        }
    }
    p
}

/// Pure ASCII PowerShell 5.1. First line OK / ERR <reason>, then the file as UTF-8.
const FETCH_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
try {
    $r = Invoke-WebRequest -Uri $env:SG_URL -UseBasicParsing -TimeoutSec 30 -Headers @{ 'User-Agent' = 'SplitGate-rules' }
    $b = $r.RawContentStream.ToArray()
    if ($b.Length -gt 5242880) { 'ERR the file is larger than 5 MB'; exit 0 }
    'OK'
    [Text.Encoding]::UTF8.GetString($b)
} catch {
    $code = 0
    if ($_.Exception.Response) { $code = [int]$_.Exception.Response.StatusCode }
    if ($code -eq 404) { 'ERR 404 (not found)' } else { 'ERR ' + $_.Exception.Message }
}
"#;

fn fetch(url: &str) -> Result<String, String> {
    let out = platform::powershell(FETCH_SCRIPT).env("SG_URL", url).output().map_err(|e| format!("无法下载：{e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (head, body) = text.split_once('\n').unwrap_or((text.as_str(), ""));
    match head.trim() {
        "OK" => Ok(body.to_string()),
        h => Err(format!("下载 {url} 失败：{}", h.trim_start_matches("ERR ").trim())),
    }
}

/// Everything one subscription brings, following `include:` next to the first file.
fn collect(url: &str, skip_ads: bool, fetch: impl Fn(&str) -> Result<String, String>) -> Result<Parsed, String> {
    let base = &url[..url.rfind('/').map(|i| i + 1).unwrap_or(url.len())];
    let mut todo = vec![url.to_string()];
    let mut seen = HashSet::new();
    let mut all = Parsed::default();
    let (mut suffix, mut keyword) = (HashSet::new(), HashSet::new());
    while let Some(u) = todo.pop() {
        if !seen.insert(u.clone()) {
            continue;
        }
        if seen.len() > MAX_FILES {
            return Err(format!("订阅引用了太多文件（超过 {MAX_FILES} 个 include）"));
        }
        let p = parse(&fetch(&u)?, skip_ads);
        all.skipped += p.skipped;
        for s in p.suffix {
            if suffix.insert(s.clone()) {
                all.suffix.push(s);
            }
        }
        for k in p.keyword {
            if keyword.insert(k.clone()) {
                all.keyword.push(k);
            }
        }
        for name in p.includes {
            if name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '!' | '@')) {
                todo.push(format!("{base}{name}"));
            }
        }
        if all.suffix.len() + all.keyword.len() > MAX_ENTRIES {
            return Err(format!("订阅的域名超过 {MAX_ENTRIES} 条，太多了（会让几乎所有网站都走 VPN），没有使用"));
        }
    }
    if all.suffix.is_empty() && all.keyword.is_empty() {
        return Err("下载到的文件里没有可用的域名（格式不对？）".into());
    }
    Ok(all)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Downloads and stores one rule set's subscription. The download runs without the file lock;
/// the result is only kept if the address did not change meanwhile.
pub fn update(id: &str) -> Result<Vec<RuleSetView>, String> {
    let sub = {
        let _guard = config::FILE_LOCK.lock().map_err(|_| "内部错误：文件锁")?;
        let file = rules::load()?;
        file.rulesets.iter().find(|r| r.id == id).and_then(|r| r.subscription.clone()).ok_or("这个规则集没有订阅")?
    };
    let url = resolve_url(&sub.url)?;
    let got = collect(&url, sub.skip_ads, fetch);
    let _guard = config::FILE_LOCK.lock().map_err(|_| "内部错误：文件锁")?;
    let mut file = rules::load()?;
    let r = file.rulesets.iter_mut().find(|r| r.id == id).ok_or("找不到这个规则集")?;
    let Some(s) = r.subscription.as_mut().filter(|s| s.url == sub.url) else { return Ok(rules::view(&file)) };
    let result = match got {
        Ok(p) => {
            let (before, after) = (r.sub_domain_suffix.len() + r.sub_domain_keyword.len(), p.suffix.len() + p.keyword.len());
            logs::info(format!("规则集“{}”的订阅已更新：{after} 条域名（之前 {before} 条），跳过 {} 行", r.name, p.skipped));
            *s = Subscription { updated_ms: now_ms(), count: after, skipped: p.skipped, error: None, ..s.clone() };
            r.sub_domain_suffix = p.suffix;
            r.sub_domain_keyword = p.keyword;
            Ok(())
        }
        Err(e) => {
            logs::warn(format!("规则集“{}”的订阅更新失败，继续用之前的 {} 条：{e}", r.name, s.count));
            s.error = Some(e.clone());
            Err(e)
        }
    };
    rules::save(&file)?;
    result.map(|_| rules::view(&file))
}

/// Ids of subscriptions whose interval has passed (or never updated); not the hand-only ones.
fn due(now: u64) -> Vec<String> {
    let Ok(_guard) = config::FILE_LOCK.lock() else { return Vec::new() };
    let Ok(file) = rules::load() else { return Vec::new() };
    file.rulesets
        .iter()
        .filter(|r| r.subscription.as_ref().is_some_and(|s| is_due(s, now)))
        .map(|r| r.id.clone())
        .collect()
}

fn is_due(s: &Subscription, now: u64) -> bool {
    s.interval_hours > 0 && now.saturating_sub(s.updated_ms) >= u64::from(s.interval_hours) * 3600 * 1000
}

/// While the app runs: a minute after start, then every 10 minutes, update what is due. Tells the
/// window which rule sets changed (`rules_updated`), so it reloads them and can ask to reconnect.
pub fn start_auto_update(app: AppHandle) {
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(60));
        loop {
            let changed: Vec<String> = due(now_ms()).into_iter().filter(|id| update(id).is_ok()).collect();
            if !changed.is_empty() {
                let _ = app.emit("rules_updated", &changed);
            }
            thread::sleep(CHECK_EVERY);
        }
    });
}

/// Called by `rules::upsert`: a changed address drops the old list (it belonged to the old address).
pub fn keep_or_clear(new: &mut RuleSet, old: Option<&RuleSet>) {
    let same = match (&new.subscription, old.and_then(|o| o.subscription.as_ref())) {
        (Some(n), Some(o)) => n.url == o.url && n.skip_ads == o.skip_ads,
        _ => false,
    };
    match (same, old) {
        (true, Some(o)) => {
            // The window may send an older copy of the downloaded list: keep the stored one, and
            // the stored update results; only the interval comes from the window.
            new.sub_domain_suffix = o.sub_domain_suffix.clone();
            new.sub_domain_keyword = o.sub_domain_keyword.clone();
            let interval = new.subscription.as_ref().map_or(DEFAULT_INTERVAL_HOURS, |s| s.interval_hours);
            new.subscription = o.subscription.clone().map(|s| Subscription { interval_hours: interval, ..s });
        }
        _ => {
            new.sub_domain_suffix.clear();
            new.sub_domain_keyword.clear();
            if let Some(s) = new.subscription.as_mut() {
                *s = Subscription { url: s.url.clone(), skip_ads: s.skip_ads, interval_hours: s.interval_hours, ..Default::default() };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_by_interval() {
        let h = 3600 * 1000;
        let s = |interval_hours, updated_ms| Subscription { interval_hours, updated_ms, ..Default::default() };
        assert!(is_due(&s(1, 0), 2 * h), "never updated");
        assert!(!is_due(&s(24, 10 * h), 20 * h));
        assert!(is_due(&s(24, 10 * h), 34 * h));
        assert!(!is_due(&s(0, 0), 1000 * h), "0 = only by hand");
        let old: Subscription = serde_json::from_str(r#"{"url":"spotify"}"#).unwrap();
        assert_eq!(old.interval_hours, 7 * 24, "files without the field: 7 days");
    }

    #[test]
    fn script_is_ascii() {
        assert!(FETCH_SCRIPT.is_ascii());
    }

    #[test]
    fn addresses() {
        assert_eq!(resolve_url("spotify").unwrap(), format!("{V2FLY_BASE}spotify"));
        assert_eq!(resolve_url(" https://example.com/list.txt ").unwrap(), "https://example.com/list.txt");
        assert!(resolve_url("http://example.com/list.txt").is_err(), "https only");
        assert!(resolve_url("file:///C:/x").is_err());
        assert!(resolve_url("a b").is_err());
    }

    #[test]
    fn reads_v2fly() {
        let p = parse("# comment\nspotify.com\nfull:audio-ak-spotify-com.akamaized.net\nkeyword:spotify\nregexp:^.+\\.spotify\\.com$\ninclude:spotify-ads\nfull:ads.spotify.com @ads\nscdn.co @cn\n", true);
        assert_eq!(p.suffix, vec!["spotify.com", "audio-ak-spotify-com.akamaized.net", "scdn.co"]);
        assert_eq!(p.keyword, vec!["spotify"]);
        assert_eq!(p.includes, vec!["spotify-ads"]);
        assert_eq!(p.skipped, 1, "regexp");
        assert_eq!(parse("full:ads.spotify.com @ads", false).suffix, vec!["ads.spotify.com"], "ads kept when asked");
    }

    #[test]
    fn reads_plain_and_clash() {
        let p = parse("*.Example.com\n.b.org\n+.c.net\nd.io // note\nnot a domain\n", true);
        assert_eq!(p.suffix, vec!["example.com", "b.org", "c.net", "d.io"]);
        assert_eq!(p.skipped, 1, "'not a domain' is not a domain");
        let c = parse("payload:\n  - DOMAIN-SUFFIX,spotify.com\n  - 'DOMAIN,api.x.com'\n  - DOMAIN-KEYWORD,spot\n  - IP-CIDR,1.2.3.0/24\n  - PROCESS-NAME,a.exe\n  - DOMAIN-SUFFIX,y.com,Proxy\n", true);
        assert_eq!(c.suffix, vec!["spotify.com", "api.x.com", "y.com"]);
        assert_eq!(c.keyword, vec!["spot"]);
        assert_eq!(c.skipped, 2, "IP and program rules never come from a subscription");
    }

    #[test]
    fn follows_includes_and_limits() {
        let files = |u: &str| -> Result<String, String> {
            Ok(match u.rsplit('/').next().unwrap() {
                "main" => "a.com\ninclude:extra\ninclude:main\n".into(),
                "extra" => "b.com\na.com\n".into(),
                _ => return Err("404".into()),
            })
        };
        let p = collect("https://x.test/data/main", true, files).unwrap();
        assert_eq!(p.suffix, vec!["a.com", "b.com"], "deduplicated; a loop back to main is read once");
        assert!(collect("https://x.test/data/missing", true, files).is_err());
        assert!(collect("https://x.test/data/main", true, |_| Ok("# nothing\n".into())).is_err(), "an empty list is an error, the old one stays");
        let big = (0..MAX_ENTRIES + 1).map(|i| format!("d{i}.com")).collect::<Vec<_>>().join("\n");
        assert!(collect("https://x.test/big", true, |_| Ok(big.clone())).is_err());
    }

    #[test]
    fn changed_address_drops_the_old_list() {
        let mut old = rules::empty_for_tests("s");
        old.subscription = Some(Subscription { url: "spotify".into(), skip_ads: true, updated_ms: 5, count: 1, ..Default::default() });
        old.sub_domain_suffix = vec!["spotify.com".into()];
        let mut same = old.clone();
        same.sub_domain_suffix.clear();
        keep_or_clear(&mut same, Some(&old));
        assert_eq!(same.sub_domain_suffix, vec!["spotify.com"], "the stored list wins over the window's copy");
        assert_eq!(same.subscription.as_ref().unwrap().updated_ms, 5);
        let mut faster = old.clone();
        faster.subscription.as_mut().unwrap().interval_hours = 1;
        keep_or_clear(&mut faster, Some(&old));
        assert_eq!(faster.subscription.as_ref().unwrap().interval_hours, 1, "a new interval is kept");
        assert_eq!(faster.sub_domain_suffix, vec!["spotify.com"], "and does not drop the list");
        let mut moved = old.clone();
        moved.subscription.as_mut().unwrap().url = "github".into();
        keep_or_clear(&mut moved, Some(&old));
        assert!(moved.sub_domain_suffix.is_empty());
        assert_eq!(moved.subscription.as_ref().unwrap().updated_ms, 0, "due for an update now");
    }
}
