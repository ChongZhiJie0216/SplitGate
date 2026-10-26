# Phase 1 acceptance: is anything left over from SplitGate? READ-ONLY.
# Checks: sing-box process, SplitGate* adapters, routes on SplitGate* or in the TUN network,
# and (with phase0\logs\baseline.json) default routes / static DNS against the baseline.
# Exit code 0 = clean, 1 = something left (fix with: phase0\cleanup-all.ps1 -Apply, admin PowerShell).
# Usage: powershell -ExecutionPolicy Bypass -File phase1\residue-check.ps1
. (Join-Path $PSScriptRoot 'common.ps1')

$base = Get-BaselineState
if ($null -eq $base) {
    Write-Host "[warn] no baseline ($BaselinePath); default route / DNS comparison skipped. Make one while disconnected: phase0\snapshot.ps1 -Baseline"
}

if (Test-Connected) { Write-Host "[warn] $TunName exists: SplitGate looks connected. Disconnect first for a meaningful check." }
$left = @(Get-Residue $base)
if ($left.Count -eq 0) { Write-Host '[clean] no SplitGate process, adapter or route left; network matches the baseline.'; exit 0 }
foreach ($i in $left) { Write-Host "[left] $i" }
Write-Host "[residue] $($left.Count) item(s). Cleanup: phase0\cleanup-all.ps1 -Apply (admin PowerShell)."
exit 1
