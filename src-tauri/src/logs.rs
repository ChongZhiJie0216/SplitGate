//! Logs (global rule 50): logs\splitgate-YYYY-MM-DD.log under the data folder, kept 30 days,
//! plus the last lines in memory for the Logs page. Every line goes through `redact`, so a
//! password or key handed to sing-box can never reach a log file or the UI.
//! In the background service (no window) lines go to the file and to a sink (the pipe); the app
//! then shows them with `relay`, without writing them a second time.

use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::{paths, platform};

const KEEP_DAYS: u64 = 30;
const MEMORY_LINES: usize = 2000;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LogLine {
    pub time: String,
    /// "app" or "sing-box"
    pub source: String,
    /// "info" / "warn" / "error"
    pub level: String,
    pub text: String,
}

static APP: OnceLock<AppHandle> = OnceLock::new();
/// The service's pipe broadcast.
type Sink = Box<dyn Fn(&LogLine) + Send + Sync>;
static SINK: OnceLock<Sink> = OnceLock::new();
/// Writing the log file starts with init() / init_service(), so unit tests stay away from the data folder.
static FILE_ON: AtomicBool = AtomicBool::new(false);
static RECENT: Mutex<VecDeque<LogLine>> = Mutex::new(VecDeque::new());
static REDACT: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn init(app: AppHandle) {
    let _ = APP.set(app);
    start_file();
}

/// The background service: file plus `sink` (each line is sent to the connected app).
// Only the Windows background service calls this.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn init_service(sink: impl Fn(&LogLine) + Send + Sync + 'static) {
    let _ = SINK.set(Box::new(sink));
    start_file();
}

fn start_file() {
    FILE_ON.store(true, Ordering::SeqCst);
    let dir = paths::logs_dir();
    let _ = fs::create_dir_all(&dir);
    // Delete log files older than KEEP_DAYS.
    if let Ok(entries) = fs::read_dir(&dir) {
        let limit = SystemTime::now() - Duration::from_secs(KEEP_DAYS * 86400);
        for e in entries.flatten() {
            let old = e.metadata().and_then(|m| m.modified()).is_ok_and(|t| t < limit);
            if old && e.file_name().to_string_lossy().ends_with(".log") {
                let _ = fs::remove_file(e.path());
            }
        }
    }
}

/// Secrets of the current connection; replaced (not appended) on every connect.
pub fn set_redactions(secrets: Vec<String>) {
    if let Ok(mut r) = REDACT.lock() {
        *r = secrets.into_iter().filter(|s| s.len() >= 4).collect();
    }
}

fn redact(text: &str) -> String {
    let mut out = text.to_string();
    if let Ok(r) = REDACT.lock() {
        for s in r.iter() {
            out = out.replace(s.as_str(), "***");
        }
    }
    out
}

pub fn info(text: impl AsRef<str>) {
    push("app", "info", text.as_ref());
}
pub fn warn(text: impl AsRef<str>) {
    push("app", "warn", text.as_ref());
}
pub fn error(text: impl AsRef<str>) {
    push("app", "error", text.as_ref());
}

/// One sing-box stderr line; the level is taken from the line itself.
pub fn singbox(line: &str) {
    let level = if line.contains("ERROR") || line.contains("FATAL") {
        "error"
    } else if line.contains("WARN") {
        "warn"
    } else {
        "info"
    };
    push("sing-box", level, line);
}

fn push(source: &'static str, level: &'static str, text: &str) {
    let (y, mo, d, h, mi, s) = platform::local_time();
    let line = LogLine { time: format!("{h:02}:{mi:02}:{s:02}"), source: source.into(), level: level.into(), text: redact(text) };
    if FILE_ON.load(Ordering::SeqCst) {
        let file = paths::logs_dir().join(format!("splitgate-{y:04}-{mo:02}-{d:02}.log"));
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(file) {
            let _ = writeln!(f, "{y:04}-{mo:02}-{d:02} {} [{source}] {level}: {}", line.time, line.text);
        }
    }
    if let Some(sink) = SINK.get() {
        sink(&line);
    }
    show(line);
}

/// A line the background service already wrote to its file: only show it.
pub fn relay(line: LogLine) {
    show(line);
}

fn show(line: LogLine) {
    if let Ok(mut r) = RECENT.lock() {
        if r.len() >= MEMORY_LINES {
            r.pop_front();
        }
        r.push_back(line.clone());
    }
    if let Some(app) = APP.get() {
        let _ = app.emit("log", &line);
    }
}

pub fn recent() -> Vec<LogLine> {
    RECENT.lock().map(|r| r.iter().cloned().collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_redacted() {
        set_redactions(vec!["synthetic-pw-123".into(), "ab".into()]);
        assert_eq!(redact("login with synthetic-pw-123 ok ab"), "login with *** ok ab", "short strings are not redacted");
        set_redactions(vec![]);
    }
}
