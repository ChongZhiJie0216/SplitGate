# Phase 1 acceptance: "disconnect and reconnect 20 times, nothing left over". READ-ONLY.
# Start it while SplitGate is DISCONNECTED (that state is the reference), then press
# Connect / Disconnect in SplitGate yourself. The script follows $TunName appearing and going,
# and after every disconnect waits up to -SettleSec for all residue to go away.
# Result log: phase1\logs\reconnect-<time>.log
# Usage: powershell -ExecutionPolicy Bypass -File phase1\reconnect-watch.ps1 [-Count 20] [-SettleSec 10]
param([int]$Count = 20, [int]$SettleSec = 10)
. (Join-Path $PSScriptRoot 'common.ps1')

if (Test-Connected) { Write-Host "[ERROR] $TunName exists. Disconnect in SplitGate first, then start this script."; exit 1 }
$first = @(Get-Residue $null)
if ($first.Count -gt 0) { foreach ($i in $first) { Write-Host "[left] $i" }; Write-Host '[ERROR] not clean before the test. Run phase0\cleanup-all.ps1 -Apply first.'; exit 1 }

$log = "reconnect-$(Get-Date -Format 'yyyyMMdd-HHmmss').log"
$base = Get-NetState
Write-Log $log "start: $Count cycles, reference default routes: $($base.DefaultRoutes -join '; ')"
Write-Host 'Now press Connect / Disconnect in SplitGate. Ctrl+C stops early.'

$bad = 0
for ($n = 1; $n -le $Count; $n++) {
    while (-not (Test-Connected)) { Start-Sleep -Milliseconds 300 }
    Write-Log $log "cycle ${n}: connected ($TunName up)"
    while (Test-Connected) { Start-Sleep -Milliseconds 300 }
    $t = [Diagnostics.Stopwatch]::StartNew()
    $left = @(Get-Residue $base)
    while ($left.Count -gt 0 -and $t.Elapsed.TotalSeconds -lt $SettleSec) {
        Start-Sleep -Milliseconds 300
        $left = @(Get-Residue $base)
    }
    if ($left.Count -eq 0) {
        Write-Log $log "cycle ${n}: disconnected, clean after $([int]$t.Elapsed.TotalMilliseconds) ms"
    } else {
        $bad++
        Write-Log $log "cycle ${n}: RESIDUE after $SettleSec s: $($left -join '; ')"
    }
}

if ($bad -eq 0) { Write-Log $log "[PASS] $Count cycles, nothing left over." }
else { Write-Log $log "[FAIL] $bad of $Count cycles left residue. Cleanup: phase0\cleanup-all.ps1 -Apply (admin PowerShell)." }
Write-Host "log: $(Join-Path $Phase1LogDir $log)"
