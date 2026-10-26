# Phase 1 acceptance: "end SplitGate in Task Manager, network is normal again within 5 s".
# Start it while SplitGate is CONNECTED. Then end SplitGate.exe in Task Manager (Details tab,
# End task) - or pass -KillNow and the script does the same (Stop-Process -Force = TerminateProcess).
# Ending SplitGate removes its TUN adapter and routes (that is the test). The script itself only
# reads state. Rollback if the network does not come back:
#   phase0\cleanup-all.ps1 -Apply   (admin PowerShell; last resort: reboot)
# "Recovered" = no sing-box, no SplitGate* adapter or route, default routes / DNS as before
# connecting, and a direct DNS lookup + TCP connect succeeds.
# Usage: powershell -ExecutionPolicy Bypass -File phase1\kill-recovery.ps1 [-KillNow] [-LimitSec 5]
param([switch]$KillNow, [int]$LimitSec = 5, [int]$WaitSec = 30)
. (Join-Path $PSScriptRoot 'common.ps1')

if (-not (Test-Connected)) { Write-Host "[ERROR] $TunName not found. Connect in SplitGate first."; exit 1 }
$app = @(Get-Process -Name 'SplitGate' -ErrorAction SilentlyContinue)
if ($app.Count -eq 0) { Write-Host '[ERROR] no SplitGate.exe process.'; exit 1 }
if ($KillNow -and -not (Test-IsAdmin)) { Write-Host '[ERROR] -KillNow needs an Administrator PowerShell (SplitGate runs as admin).'; exit 1 }

$base = Get-BaselineState
if ($null -eq $base) {
    Write-Host '[warn] no phase0\logs\baseline.json: default route / DNS not compared (make one while disconnected: phase0\snapshot.ps1 -Baseline).'
}

$log = "kill-recovery-$(Get-Date -Format 'yyyyMMdd-HHmmss').log"
Write-Log $log "connected, SplitGate pid $($app.Id -join ',')"
if ($KillNow) {
    Write-Log $log 'ending SplitGate.exe (Stop-Process -Force)'
    Stop-Process -Id $app.Id -Force
} else {
    Write-Host 'Now end SplitGate.exe in Task Manager (Details tab > End task). Waiting...'
}
while (@(Get-Process -Id $app.Id -ErrorAction SilentlyContinue).Count -gt 0) { Start-Sleep -Milliseconds 50 }
$t = [Diagnostics.Stopwatch]::StartNew()
Write-Log $log 'SplitGate.exe is gone, timing recovery'

$left = @('not checked yet'); $net = $false
while ($t.Elapsed.TotalSeconds -lt $WaitSec) {
    $left = @(Get-Residue $base)
    $net = Test-DirectNet
    if ($left.Count -eq 0 -and $net) { break }
    Start-Sleep -Milliseconds 100
}
$ms = [int]$t.Elapsed.TotalMilliseconds
if ($left.Count -eq 0 -and $net) {
    if ($ms -le $LimitSec * 1000) { Write-Log $log "[PASS] network normal $ms ms after SplitGate ended (limit $LimitSec s)." }
    else { Write-Log $log "[FAIL] network normal only after $ms ms (limit $LimitSec s)." }
} else {
    Write-Log $log "[FAIL] not recovered after $WaitSec s. direct net ok: $net. left: $($left -join '; ')"
    Write-Host 'Rollback: phase0\cleanup-all.ps1 -Apply (admin PowerShell).'
}
Write-Host "log: $(Join-Path $Phase1LogDir $log)"
