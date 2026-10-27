//! Cleanup after disconnect, crash or forced exit (SPEC "错误处理与清理"), same checks as
//! phase0\cleanup-all.ps1: our own sing-box (matched by full path, never another program's),
//! routes on and adapters named SplitGate*, then a DNS cache flush. Without `apply` it only reports.
//! Other VPN apps' adapters (e.g. Pangolin) are never touched.

use serde::Serialize;

use crate::{logs, paths, platform};

/// Pure ASCII PowerShell 5.1. Inputs come from environment variables, so nothing needs quoting.
const SCRIPT: &str = r#"
$ErrorActionPreference = 'Continue'
$apply = $env:SG_APPLY -eq '1'
$sb = @($env:SG_SINGBOX -split ';' | Where-Object { $_ })
foreach ($p in @(Get-Process -Name 'sing-box' -ErrorAction SilentlyContinue)) {
    if ($p.Path -and ($sb -contains $p.Path)) {
        Write-Output "found: sing-box pid $($p.Id)"
        if ($apply) {
            try { Stop-Process -Id $p.Id -Force -ErrorAction Stop; Start-Sleep -Milliseconds 800; Write-Output "did: stopped sing-box pid $($p.Id)" }
            catch { Write-Output "fail: stop sing-box pid $($p.Id): $($_.Exception.Message)" }
        }
    }
}
foreach ($r in @(Get-NetRoute -ErrorAction SilentlyContinue | Where-Object { $_.InterfaceAlias -like 'SplitGate*' })) {
    $what = "route $($r.DestinationPrefix) on $($r.InterfaceAlias)"
    Write-Output "found: $what"
    if ($apply) {
        try { $r | Remove-NetRoute -Confirm:$false -ErrorAction Stop; Write-Output "did: removed $what" }
        catch { Write-Output "fail: remove ${what}: $($_.Exception.Message)" }
    }
}
foreach ($a in @(Get-NetAdapter -IncludeHidden -Name 'SplitGate*' -ErrorAction SilentlyContinue)) {
    Write-Output "found: adapter $($a.Name)"
    if ($apply) {
        $out = pnputil /remove-device "$($a.PnPDeviceID)" 2>&1
        if ($LASTEXITCODE -eq 0) { Write-Output "did: removed adapter $($a.Name)" } else { Write-Output "fail: remove adapter $($a.Name): $out" }
    }
}
if ($apply) { ipconfig /flushdns | Out-Null }
"#;

#[derive(Serialize, Default, Debug)]
pub struct Report {
    pub found: Vec<String>,
    pub done: Vec<String>,
    pub failed: Vec<String>,
}

impl Report {
    pub fn is_clean(&self) -> bool {
        self.found.is_empty()
    }
}

pub fn run(apply: bool) -> Result<Report, String> {
    let apply = apply && platform::is_admin();
    let out = platform::powershell(SCRIPT)
        .env("SG_APPLY", if apply { "1" } else { "0" })
        .env("SG_SINGBOX", paths::our_singbox_exes().iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(";"))
        .output()
        .map_err(|e| format!("无法执行清理：{e}"))?;
    let mut rep = Report::default();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some(s) = line.strip_prefix("found: ") {
            rep.found.push(s.to_string());
        } else if let Some(s) = line.strip_prefix("did: ") {
            rep.done.push(s.to_string());
        } else if let Some(s) = line.strip_prefix("fail: ") {
            rep.failed.push(s.to_string());
        }
    }
    for s in &rep.done {
        logs::info(format!("清理：{s}"));
    }
    for s in &rep.failed {
        logs::error(format!("清理失败：{s}"));
    }
    if !apply && !rep.found.is_empty() {
        logs::warn(format!("发现残留（没有管理员权限，未清理）：{}", rep.found.join("；")));
    }
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_is_ascii() {
        assert!(SCRIPT.is_ascii(), "PowerShell 5.1 script must be pure ASCII");
    }

    /// Check mode only: reads the system, changes nothing.
    #[test]
    fn check_mode_runs() {
        let rep = run(false).unwrap();
        assert!(rep.done.is_empty() && rep.failed.is_empty());
    }
}
