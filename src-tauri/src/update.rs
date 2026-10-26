//! "检查更新" (maintainer's choice 2026-10-26): only tells that a newer version exists and shows its
//! CHANGELOG; nothing is downloaded or installed (a real updater comes after the Windows service,
//! with a signed NSIS installer). Source: the latest GitHub Release of this repo, and CHANGELOG.md
//! at that release's tag. Fetched with Windows PowerShell; no extra dependency. Read-only.

use serde::Serialize;
use serde_json::Value;

use crate::{logs, platform};

pub const REPO: &str = "ChongZhiJie0216/SplitGate";

/// Pure ASCII PowerShell 5.1. Prints one JSON object: {tag, url, changelog} or {none} / {error}.
const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$headers = @{ 'User-Agent' = 'SplitGate-update-check'; 'Accept' = 'application/vnd.github+json' }
try {
    $r = Invoke-RestMethod -Uri "https://api.github.com/repos/$env:SG_REPO/releases/latest" -Headers $headers -TimeoutSec 15
} catch {
    $code = 0
    if ($_.Exception.Response) { $code = [int]$_.Exception.Response.StatusCode }
    if ($code -eq 404) { @{ none = $true } | ConvertTo-Json -Compress; exit 0 }
    @{ error = $_.Exception.Message } | ConvertTo-Json -Compress; exit 0
}
$log = ''
try {
    $w = Invoke-WebRequest -Uri "https://raw.githubusercontent.com/$env:SG_REPO/$($r.tag_name)/CHANGELOG.md" -Headers @{ 'User-Agent' = 'SplitGate-update-check' } -UseBasicParsing -TimeoutSec 15
    $log = [Text.Encoding]::UTF8.GetString($w.RawContentStream.ToArray())
} catch { $log = '' }
@{ tag = [string]$r.tag_name; url = [string]$r.html_url; changelog = $log } | ConvertTo-Json -Compress
"#;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Section {
    pub version: String,
    /// The rest of the heading, e.g. "2026-10-26（流量画布）".
    pub title: String,
    /// Markdown body of that version.
    pub body: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct UpdateInfo {
    pub current: String,
    /// None when the repo has no release yet.
    pub latest: Option<String>,
    pub newer: bool,
    /// The release page (only ever a page of this repo).
    pub url: Option<String>,
    /// Versions newer than the current one, newest first.
    pub notes: Vec<Section>,
}

pub fn check(current: &str) -> Result<UpdateInfo, String> {
    let out = platform::powershell(SCRIPT).env("SG_REPO", REPO).output().map_err(|e| format!("无法检查更新：{e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let v: Value = serde_json::from_str(text.trim()).map_err(|_| "检查更新失败：GitHub 的回应看不懂".to_string())?;
    if let Some(e) = v["error"].as_str() {
        return Err(format!("检查更新失败（连不到 GitHub？）：{e}"));
    }
    let info = interpret(current, &v);
    if info.newer {
        logs::info(format!("有新版本：{}（现在是 {current}）", info.latest.as_deref().unwrap_or("")));
    }
    Ok(info)
}

fn interpret(current: &str, v: &Value) -> UpdateInfo {
    let none = UpdateInfo { current: current.into(), latest: None, newer: false, url: None, notes: Vec::new() };
    let Some(tag) = v["tag"].as_str() else { return none };
    let latest = tag.trim_start_matches('v').to_string();
    let newer = matches!((parse(&latest), parse(current)), (Some(l), Some(c)) if l > c);
    let release_page = format!("https://github.com/{REPO}/releases/");
    let url = v["url"].as_str().filter(|u| u.starts_with(&release_page)).map(str::to_string);
    let notes = if newer { sections_newer_than(v["changelog"].as_str().unwrap_or(""), current) } else { Vec::new() };
    UpdateInfo { latest: Some(latest), newer, url, notes, ..none }
}

fn parse(v: &str) -> Option<(u32, u32, u32)> {
    let mut p = v.trim().trim_start_matches('v').split('.').map(|x| x.parse::<u32>().ok());
    match (p.next()??, p.next()??, p.next()??, p.next()) {
        (a, b, c, None) => Some((a, b, c)),
        _ => None,
    }
}

/// CHANGELOG.md sections "## x.y.z — title" with a version above `current`, in file order.
fn sections_newer_than(changelog: &str, current: &str) -> Vec<Section> {
    let Some(cur) = parse(current) else { return Vec::new() };
    let mut out: Vec<Section> = Vec::new();
    let mut open = false;
    for line in changelog.lines() {
        if let Some(head) = line.strip_prefix("## ") {
            let (ver, title) = head.split_once(' ').unwrap_or((head, ""));
            open = parse(ver).is_some_and(|v| v > cur);
            if open {
                let title = title.trim_start_matches(['—', '-', ' ']).trim().to_string();
                out.push(Section { version: ver.to_string(), title, body: String::new() });
            }
        } else if open {
            if let Some(s) = out.last_mut() {
                s.body.push_str(line);
                s.body.push('\n');
            }
        }
    }
    for s in &mut out {
        s.body = s.body.trim().to_string();
    }
    out
}

/// Opens the release page in the default browser; nothing outside this repo's releases.
pub fn open_release_page(url: &str) -> Result<(), String> {
    let ok = url.starts_with(&format!("https://github.com/{REPO}/releases/")) && !url.contains(['"', ' ', '\n']);
    if !ok {
        return Err("只能打开 SplitGate 的发布页".into());
    }
    std::process::Command::new("explorer.exe").arg(url).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_is_ascii() {
        assert!(SCRIPT.is_ascii());
    }

    const LOG: &str = "# 更新记录\n\n说明\n\n## 未发布\n\n## 0.13.0 — 2026-10-27（检查更新）\n\n### 新增\n- 检查更新\n\n## 0.12.0 — 2026-10-26（设置放在程序旁边）\n\n### 改变\n- data 文件夹\n\n## 0.11.1 — 2026-10-26（断网检测更快）\n\n### 修正\n- 快\n";

    #[test]
    fn versions_compare_as_numbers() {
        assert!(parse("0.10.0") > parse("0.9.9"));
        assert_eq!(parse("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse("1.2"), None);
        assert_eq!(parse("未发布"), None);
    }

    #[test]
    fn only_newer_sections() {
        let s = sections_newer_than(LOG, "0.11.1");
        assert_eq!(s.iter().map(|x| x.version.as_str()).collect::<Vec<_>>(), ["0.13.0", "0.12.0"]);
        assert_eq!(s[0].title, "2026-10-27（检查更新）");
        assert_eq!(s[0].body, "### 新增\n- 检查更新");
        assert!(sections_newer_than(LOG, "0.13.0").is_empty());
    }

    #[test]
    fn interprets_release() {
        let v = serde_json::json!({"tag": "v0.13.0", "url": format!("https://github.com/{REPO}/releases/tag/v0.13.0"), "changelog": LOG});
        let i = interpret("0.12.0", &v);
        assert!(i.newer);
        assert_eq!(i.latest.as_deref(), Some("0.13.0"));
        assert_eq!(i.notes.len(), 1);
        assert!(i.url.is_some());
        let same = interpret("0.13.0", &v);
        assert!(!same.newer && same.notes.is_empty());
        let none = interpret("0.12.0", &serde_json::json!({"none": true}));
        assert_eq!(none.latest, None);
        let odd = interpret("0.12.0", &serde_json::json!({"tag": "v0.13.0", "url": "https://evil.example/x"}));
        assert_eq!(odd.url, None, "only this repo's release pages");
    }

    #[test]
    fn release_page_only() {
        assert!(open_release_page("https://evil.example/").is_err());
        assert!(open_release_page("https://github.com/someone/else/releases/x").is_err());
    }
}
