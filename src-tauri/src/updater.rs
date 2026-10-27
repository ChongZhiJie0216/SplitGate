//! Online update of the portable copy (v0.19.0, maintainer's choice 2026-10-27: portable, ask
//! before disconnecting). Only when the user asks (About page):
//! 1. download the release's portable zip into `data\update\`, check it against the release's
//!    `.sha256` file, unpack it (Windows PowerShell, no extra dependency);
//! 2. install: disconnect first, then replace SplitGate.exe, sing-box.exe (and `licenses\` when this
//!    copy has one). A running exe cannot be overwritten but can be renamed: the old files become
//!    `*.old` and the new ones are copied in; any failure puts the old ones back. With the
//!    background service the service runs this exe too: it is stopped and started again around the
//!    copy, in an elevated copy of SplitGate (elevate.rs) unless this one is administrator;
//! 3. start the new SplitGate (`--wait-pid`) and quit. The new copy deletes the `*.old` files and
//!    `data\update\` when it starts, so until then the old version can be put back by hand.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::supervisor::Status;
use crate::update::Download;
use crate::{backend, elevate, logs, paths, platform, service};

/// Replaced on update. `licenses\` only when the copy already has that folder (the copy that
/// build.bat puts in the repo root has none).
pub const FILES: [&str; 2] = ["SplitGate.exe", "sing-box.exe"];
const LICENSES: &str = "licenses";

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Job {
    Idle,
    /// `total` 0 = size unknown.
    Downloading { version: String, done: u64, total: u64 },
    Ready { version: String },
    Installing { version: String },
    Failed { message: String },
}

static JOB: Mutex<Job> = Mutex::new(Job::Idle);

pub fn job() -> Job {
    JOB.lock().map(|j| j.clone()).unwrap_or(Job::Idle)
}

fn set(j: Job) {
    if let Ok(mut g) = JOB.lock() {
        *g = j;
    }
}

pub fn update_dir() -> PathBuf {
    paths::data_dir().join("update")
}

/// Where a downloaded version is unpacked; the zip holds one folder, `SplitGate\`.
fn unpacked(version: &str) -> PathBuf {
    update_dir().join(version).join("SplitGate")
}

/// Pure ASCII PowerShell 5.1. Prints {ok} or {error} (or {mismatch} for a bad checksum).
const DOWNLOAD_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$h = @{ 'User-Agent' = 'SplitGate-updater' }
try {
    $s = Invoke-WebRequest -Uri $env:SG_SHA_URL -Headers $h -UseBasicParsing -TimeoutSec 30
    $text = if ($s.Content -is [byte[]]) { [Text.Encoding]::ASCII.GetString($s.Content) } else { [string]$s.Content }
    $want = ($text.Trim() -split '\s+')[0].ToLower()
    if ($want -notmatch '^[0-9a-f]{64}$') { @{ error = 'the .sha256 file has no SHA256' } | ConvertTo-Json -Compress; exit 0 }
    Invoke-WebRequest -Uri $env:SG_ZIP_URL -Headers $h -UseBasicParsing -OutFile $env:SG_ZIP -TimeoutSec 1800
    $got = (Get-FileHash -Algorithm SHA256 -LiteralPath $env:SG_ZIP).Hash.ToLower()
    if ($got -ne $want) { @{ mismatch = $true } | ConvertTo-Json -Compress; exit 0 }
    if (Test-Path -LiteralPath $env:SG_DIR) { Remove-Item -LiteralPath $env:SG_DIR -Recurse -Force }
    Expand-Archive -LiteralPath $env:SG_ZIP -DestinationPath $env:SG_DIR -Force
    @{ ok = $true } | ConvertTo-Json -Compress
} catch {
    @{ error = $_.Exception.Message } | ConvertTo-Json -Compress
}
"#;

/// Starts downloading in the background; `job()` shows the progress.
pub fn start_download(d: Download) -> Result<(), String> {
    {
        let mut g = JOB.lock().map_err(|_| "内部错误")?;
        if matches!(*g, Job::Downloading { .. } | Job::Installing { .. }) {
            return Err("已经在下载或安装中".into());
        }
        *g = Job::Downloading { version: d.version.clone(), done: 0, total: d.size };
    }
    thread::spawn(move || {
        let r = download(&d);
        match &r {
            Ok(()) => {
                logs::info(format!("新版本 v{} 已下载并核对无误", d.version));
                set(Job::Ready { version: d.version.clone() });
            }
            Err(e) => {
                logs::error(format!("下载新版本失败：{e}"));
                set(Job::Failed { message: e.clone() });
            }
        }
    });
    Ok(())
}

fn download(d: &Download) -> Result<(), String> {
    let dir = update_dir();
    // One download at a time: older leftovers go.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("无法建立 {}：{e}", dir.display()))?;
    let zip = dir.join(format!("SplitGate-{}-portable.zip", d.version));
    logs::info(format!("开始下载新版本 v{}（{} MB）", d.version, d.size / 1_000_000));
    let mut child = platform::powershell(DOWNLOAD_SCRIPT)
        .env("SG_ZIP_URL", &d.zip_url)
        .env("SG_SHA_URL", &d.sha_url)
        .env("SG_ZIP", &zip)
        .env("SG_DIR", dir.join(&d.version))
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法开始下载：{e}"))?;
    // The zip grows while it downloads: its size is the progress.
    while child.try_wait().map_err(|e| e.to_string())?.is_none() {
        let done = std::fs::metadata(&zip).map(|m| m.len()).unwrap_or(0);
        set(Job::Downloading { version: d.version.clone(), done, total: d.size });
        thread::sleep(Duration::from_millis(300));
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).map_err(|_| "下载程序的回应看不懂".to_string())?;
    if v["mismatch"].as_bool() == Some(true) {
        let _ = std::fs::remove_dir_all(&dir);
        return Err("下载的文件和发布页的校验值不一样（下载不完整或被改过），已删掉。可以再试一次。".into());
    }
    if let Some(e) = v["error"].as_str() {
        return Err(format!("{e}（连不到 GitHub？）"));
    }
    let new = unpacked(&d.version);
    if let Some(f) = FILES.iter().find(|f| !new.join(f).is_file()) {
        return Err(format!("下载的压缩包里没有 {f}"));
    }
    let _ = std::fs::remove_file(&zip);
    Ok(())
}

/// Installs the downloaded version and starts it; the caller then quits the app. Disconnects
/// first (the About page asked the user when connected).
pub fn install() -> Result<(), String> {
    let Job::Ready { version } = job() else { return Err("还没有下载好新版本".into()) };
    set(Job::Installing { version: version.clone() });
    let r = install_ready(&version);
    if let Err(e) = &r {
        logs::error(format!("安装新版本失败：{e}"));
        set(Job::Failed { message: e.clone() });
    }
    r
}

fn install_ready(version: &str) -> Result<(), String> {
    if !matches!(backend::status(), Status::Disconnected | Status::Error { .. }) {
        logs::info("更新前先断开连接");
        backend::disconnect()?;
        let end = Instant::now() + Duration::from_secs(15);
        while !matches!(backend::status(), Status::Disconnected | Status::Error { .. }) {
            if Instant::now() > end {
                return Err("15 秒内没能断开连接，没有更新".into());
            }
            thread::sleep(Duration::from_millis(200));
        }
    }
    let src = unpacked(version);
    let svc = service::state();
    let with_service = svc.installed && svc.matches_this_exe;
    logs::info(format!("安装新版本 v{version}{}", if with_service { "（先停后台服务，换好再启动）" } else { "" }));
    if with_service && !platform::is_admin() {
        elevate::run(&elevate::Task::ApplyUpdate { src: src.display().to_string() })?;
    } else if with_service {
        apply_with_service(&src)?;
    } else {
        match replace_files(&src, &paths::exe_dir()) {
            Ok(()) => {}
            // e.g. a folder only administrators may write to
            Err(_) if !platform::is_admin() => elevate::run(&elevate::Task::ApplyUpdate { src: src.display().to_string() })?,
            Err(e) => return Err(e),
        }
    }
    let exe = paths::exe_dir().join("SplitGate.exe");
    std::process::Command::new(&exe)
        .args([elevate::WAIT_PID_ARG, &std::process::id().to_string()])
        .spawn()
        .map_err(|e| format!("新版本已经换好，但启动不了（{e}）。请自己打开 {}", exe.display()))?;
    logs::info(format!("已换成 v{version}，正在重新启动"));
    Ok(())
}

/// In the elevated copy (`--admin-task apply-update`) or an administrator app: the service runs
/// this exe, so stop it around the copy. `src` must be an unpacked download of this copy.
pub fn apply_with_service(src: &Path) -> Result<(), String> {
    let base = update_dir().canonicalize().map_err(|_| "没有下载好的新版本".to_string())?;
    let src = src.canonicalize().map_err(|_| "没有下载好的新版本".to_string())?;
    if !src.starts_with(&base) {
        return Err("新版本只能来自 data\\update".into());
    }
    let protected = service::state().protected;
    let was_running = service::stop_installed()?;
    let mut r = replace_files(&src, &paths::exe_dir());
    // Since v0.20.0 the service runs its own copy: give it the new files as well.
    if r.is_ok() && protected {
        r = service::copy_to_service_dir();
    }
    // Started again either way: with the new files, or with the old ones put back.
    if was_running {
        service::start_installed()?;
    }
    r
}

/// Replace FILES (and `licenses\` when `dst` has one) in `dst` with those in `src`; on any
/// failure everything already replaced is put back.
pub fn replace_files(src: &Path, dst: &Path) -> Result<(), String> {
    let mut names: Vec<PathBuf> = FILES.iter().map(PathBuf::from).collect();
    if dst.join(LICENSES).is_dir() {
        if let Ok(entries) = std::fs::read_dir(src.join(LICENSES)) {
            names.extend(entries.flatten().filter(|e| e.path().is_file()).map(|e| Path::new(LICENSES).join(e.file_name())));
        }
    }
    // (target, its old copy if there was one)
    let mut done: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
    for name in &names {
        let (from, to) = (src.join(name), dst.join(name));
        let step = || -> Result<Option<PathBuf>, String> {
            let old = if to.exists() {
                let old = free_old_name(&to);
                std::fs::rename(&to, &old).map_err(|e| format!("{} 改名失败：{e}", to.display()))?;
                Some(old)
            } else {
                None
            };
            if let Err(e) = std::fs::copy(&from, &to) {
                if let Some(o) = &old {
                    let _ = std::fs::rename(o, &to);
                }
                return Err(format!("复制 {} 失败：{e}", name.display()));
            }
            Ok(old)
        };
        match step() {
            Ok(old) => done.push((to, old)),
            Err(e) => {
                for (to, old) in done.into_iter().rev() {
                    let _ = std::fs::remove_file(&to);
                    if let Some(o) = old {
                        let _ = std::fs::rename(o, &to);
                    }
                }
                return Err(format!("{e}；已经换回原来的文件"));
            }
        }
    }
    Ok(())
}

/// `x.exe.old`, or `x.exe.old2`… when an earlier one is still locked (its process still running).
fn free_old_name(path: &Path) -> PathBuf {
    let base = path.as_os_str().to_owned();
    for n in 1.. {
        let mut name = base.clone();
        name.push(if n == 1 { ".old".to_string() } else { format!(".old{n}") });
        let p = PathBuf::from(name);
        if !p.exists() || std::fs::remove_file(&p).is_ok() {
            return p;
        }
    }
    unreachable!()
}

/// "x.exe.old" / "x.exe.old2" → "x.exe".
fn old_copy_of(name: &str) -> Option<&str> {
    let i = name.rfind(".old")?;
    name[i + 4..].chars().all(|c| c.is_ascii_digit()).then(|| &name[..i])
}

/// Only what replace_files leaves: in the program folder (maybe the repo root) just the old
/// SplitGate.exe / sing-box.exe, in `licenses\` any old file.
fn is_old_copy(name: &str, in_licenses: bool) -> bool {
    old_copy_of(name).is_some_and(|base| in_licenses || FILES.contains(&base))
}

/// At startup of the new version: the old files and the download are no longer needed. The old
/// exe may still be running for a moment (the service, the old window), so a few tries.
pub fn clean_leftovers() {
    let dir = paths::exe_dir();
    let lic = dir.join(LICENSES);
    if !has_leftovers(&dir, &lic) && !update_dir().exists() {
        return;
    }
    thread::spawn(move || {
        for _ in 0..10 {
            let mut left = false;
            for (d, in_lic) in [(&dir, false), (&lic, true)] {
                for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                    if is_old_copy(&e.file_name().to_string_lossy(), in_lic) && std::fs::remove_file(e.path()).is_err() {
                        left = true;
                    }
                }
            }
            // Not while a new download is going on.
            if matches!(job(), Job::Idle | Job::Failed { .. }) && update_dir().exists() && std::fs::remove_dir_all(update_dir()).is_err() {
                left = true;
            }
            if !left {
                logs::info("已删掉更新前的旧文件");
                return;
            }
            thread::sleep(Duration::from_secs(3));
        }
    });
}

/// The `*.old` copies replace_files left in `dir` (not in use any more); best effort.
// Only the Windows service installer calls this.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn remove_old_copies(dir: &Path) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if is_old_copy(&e.file_name().to_string_lossy(), false) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

fn has_leftovers(dir: &Path, lic: &Path) -> bool {
    [(dir, false), (lic, true)]
        .iter()
        .any(|(d, in_lic)| std::fs::read_dir(d).into_iter().flatten().flatten().any(|e| is_old_copy(&e.file_name().to_string_lossy(), *in_lic)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_is_ascii() {
        assert!(DOWNLOAD_SCRIPT.is_ascii());
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("splitgate-updater-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(p: &Path, text: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    fn read(p: &Path) -> String {
        std::fs::read_to_string(p).unwrap()
    }

    #[test]
    fn replaces_and_keeps_old_copies() {
        let (src, dst) = (tmp("src1"), tmp("dst1"));
        for f in FILES {
            write(&src.join(f), &format!("new {f}"));
            write(&dst.join(f), &format!("old {f}"));
        }
        write(&src.join("licenses/a.txt"), "new a");
        write(&dst.join("licenses/a.txt"), "old a");
        write(&dst.join("data/profiles.json"), "mine");
        replace_files(&src, &dst).unwrap();
        assert_eq!(read(&dst.join("SplitGate.exe")), "new SplitGate.exe");
        assert_eq!(read(&dst.join("SplitGate.exe.old")), "old SplitGate.exe");
        assert_eq!(read(&dst.join("licenses/a.txt")), "new a");
        assert_eq!(read(&dst.join("data/profiles.json")), "mine", "data\\ is never touched");
        // a second update while the first .old is still there
        write(&src.join("SplitGate.exe"), "newer");
        replace_files(&src, &dst).unwrap();
        assert_eq!(read(&dst.join("SplitGate.exe")), "newer");
    }

    #[test]
    fn no_licenses_folder_is_created() {
        let (src, dst) = (tmp("src2"), tmp("dst2"));
        for f in FILES {
            write(&src.join(f), "new");
            write(&dst.join(f), "old");
        }
        write(&src.join("licenses/a.txt"), "new a");
        replace_files(&src, &dst).unwrap();
        assert!(!dst.join("licenses").exists(), "the repo-root copy has no licenses folder");
    }

    #[test]
    fn failure_puts_the_old_files_back() {
        let (src, dst) = (tmp("src3"), tmp("dst3"));
        write(&src.join("SplitGate.exe"), "new");
        // sing-box.exe missing from the download: the copy fails half way
        for f in FILES {
            write(&dst.join(f), &format!("old {f}"));
        }
        assert!(replace_files(&src, &dst).is_err());
        assert_eq!(read(&dst.join("SplitGate.exe")), "old SplitGate.exe");
        assert_eq!(read(&dst.join("sing-box.exe")), "old sing-box.exe");
    }

    #[test]
    fn old_copies_are_recognised() {
        for n in ["SplitGate.exe.old", "SplitGate.exe.old2", "sing-box.exe.old"] {
            assert!(is_old_copy(n, false), "{n}");
        }
        for n in ["SplitGate.exe", "data", "sing-box.exe.older", "LICENSE", "notes.txt.old"] {
            assert!(!is_old_copy(n, false), "{n}: not ours in the program folder");
        }
        assert!(is_old_copy("sing-box-LICENSE.txt.old", true));
    }
}
