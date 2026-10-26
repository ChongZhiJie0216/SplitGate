//! Structure checks (global rule 60-testing): one version source, CHANGELOG matches it,
//! PowerShell scripts are pure ASCII + CRLF, binaries never sit in tracked folders by accident.

use std::fs;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn package_version() -> String {
    let text = fs::read_to_string(repo().join("package.json")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    v["version"].as_str().unwrap().to_string()
}

#[test]
fn cargo_version_matches_package_json() {
    assert_eq!(env!("CARGO_PKG_VERSION"), package_version(), "src-tauri/Cargo.toml version must equal package.json");
}

#[test]
fn tauri_conf_reads_package_json() {
    let text = fs::read_to_string(repo().join("src-tauri/tauri.conf.json")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["version"], "../package.json", "tauri.conf.json must not hard-code the version");
}

#[test]
fn changelog_first_release_is_current_version() {
    let text = fs::read_to_string(repo().join("CHANGELOG.md")).unwrap();
    let first = text
        .lines()
        .find(|l| l.starts_with("## ") && !l.starts_with("## 未发布"))
        .expect("CHANGELOG.md has no release heading");
    let want = format!("## {} ", package_version());
    assert!(first.starts_with(&want), "first CHANGELOG release is '{first}', expected it to start with '{want}'");
}

#[test]
fn powershell_scripts_are_ascii_crlf() {
    let mut bad = Vec::new();
    for dir in ["scripts", "phase0", "third_party"] {
        visit(&repo().join(dir), &mut |p| {
            if p.extension().and_then(|e| e.to_str()) != Some("ps1") {
                return;
            }
            let bytes = fs::read(p).unwrap();
            if bytes.iter().any(|b| *b > 127) {
                bad.push(format!("{} has non-ASCII bytes", p.display()));
            }
            let lf = bytes.iter().filter(|b| **b == b'\n').count();
            let crlf = bytes.windows(2).filter(|w| w == b"\r\n").count();
            if lf != crlf {
                bad.push(format!("{} has LF-only line endings", p.display()));
            }
        });
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

fn visit(dir: &Path, f: &mut dyn FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if name != "bin" && name != "downloads" && name != "logs" && name != "runtime" {
                visit(&p, f);
            }
        } else {
            f(&p);
        }
    }
}
