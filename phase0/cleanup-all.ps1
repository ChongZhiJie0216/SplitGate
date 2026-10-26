# Emergency cleanup: bring the network back to the baseline after any phase0 test.
# Default is CHECK mode: only prints what it would do. Add -Apply (as Administrator) to really do it.
# Usage:
#   powershell -ExecutionPolicy Bypass -File phase0\cleanup-all.ps1            (check only)
#   powershell -ExecutionPolicy Bypass -File phase0\cleanup-all.ps1 -Apply     (admin PowerShell)
# Last resort if this script cannot fix it: Device Manager > Network adapters > uninstall every
# "SplitGate*" / "Wintun" adapter, then reboot.
param([switch]$Apply)
. (Join-Path $PSScriptRoot 'common.ps1')
$ErrorActionPreference = 'Continue'

if ($Apply -and -not (Test-IsAdmin)) { Write-Host '[ERROR] -Apply needs an Administrator PowerShell.'; exit 1 }
$mode = if ($Apply) { 'APPLY' } else { 'CHECK' }
Write-Host "=== SplitGate phase0 cleanup ($mode) ==="
$todo = 0

function Step([string]$what, [scriptblock]$action) {
    $script:todo++
    if ($Apply) {
        Write-Host "[do]   $what"
        try { & $action } catch { Write-Host "[fail] $what : $($_.Exception.Message)" }
    } else {
        Write-Host "[would] $what"
    }
}

# 1. Processes (sing-box first, it owns the TUN and system routes; then the exits)
foreach ($name in $OurProcessNames) {
    foreach ($p in @(Get-Process -Name $name -ErrorAction SilentlyContinue)) {
        $pid_ = $p.Id
        Step "stop process $($p.ProcessName) (pid $pid_) $($p.Path)" { Stop-Process -Id $pid_ -Force; Start-Sleep -Milliseconds 800 }
    }
}

# 2. Routes on SplitGate adapters
foreach ($r in @(Get-NetRoute -ErrorAction SilentlyContinue | Where-Object { $_.InterfaceAlias -like $OurAdapterPattern })) {
    $route = $r
    Step "remove route $($r.DestinationPrefix) via $($r.NextHop) on $($r.InterfaceAlias)" { $route | Remove-NetRoute -Confirm:$false }
}

# 3. SplitGate adapters (wintun adapters left behind by a crash)
foreach ($a in @(Get-OurAdapters)) {
    $dev = $a.PnPDeviceID
    Step "remove adapter '$($a.Name)' ($($a.InterfaceDescription)) device $dev" { pnputil /remove-device "$dev" | Out-Host }
}

# 4. Compare with baseline
if (Test-Path $BaselinePath) {
    $base = Get-Content $BaselinePath -Raw | ConvertFrom-Json

    # 4a. Static DNS on interfaces that existed at baseline
    $now = @(Get-StaticDnsState)
    foreach ($b in @($base.StaticDns)) {
        $cur = $now | Where-Object { $_.Guid -eq $b.Guid } | Select-Object -First 1
        if ($null -eq $cur -or $cur.Alias -like $OurAdapterPattern) { continue }
        if ([string]$cur.NameServer -ne [string]$b.NameServer) {
            $alias = $cur.Alias; $want = [string]$b.NameServer
            if ($want -eq '') {
                Step "reset DNS on '$alias' to automatic (now: $($cur.NameServer))" { Set-DnsClientServerAddress -InterfaceAlias $alias -ResetServerAddresses }
            } else {
                $list = $want -split '[ ,]+' | Where-Object { $_ }
                Step "restore DNS on '$alias' to $want (now: $($cur.NameServer))" { Set-DnsClientServerAddress -InterfaceAlias $alias -ServerAddresses $list }
            }
        }
    }

    # 4b. Default routes that were not there at baseline (only reported: they may belong to other software)
    $baseKeys = @($base.DefaultRoutes | ForEach-Object { "$($_.DestinationPrefix)|$($_.NextHop)|$($_.InterfaceAlias)" })
    foreach ($r in @(Get-DefaultRoutes)) {
        $k = "$($r.DestinationPrefix)|$($r.NextHop)|$($r.InterfaceAlias)"
        if ($baseKeys -notcontains $k -and $r.InterfaceAlias -notlike $OurAdapterPattern) {
            Write-Host "[warn] new default route not in baseline: $k  (not removed automatically - check it)"
        }
    }

    # 4c. NRPT rules (only reported)
    foreach ($n in @(Get-DnsClientNrptRule -ErrorAction SilentlyContinue)) {
        if (@($base.NrptRules) -notcontains $n.Name) { Write-Host "[warn] new NRPT rule not in baseline: $($n.Name) $($n.Namespace)" }
    }
} else {
    Write-Host "[warn] no baseline at $BaselinePath - run phase0\snapshot.ps1 first. DNS and route comparison skipped."
}

# 5. Runtime files (generated configs may contain keys or passwords)
$runtimeFiles = @()
if (Test-Path $RuntimeDir) { $runtimeFiles += Get-ChildItem $RuntimeDir -File -Recurse }
$tunnelJson = Join-Path $env:LOCALAPPDATA 'SplitGate\tunnel.json'
if (Test-Path $tunnelJson) { $runtimeFiles += Get-Item $tunnelJson }
foreach ($f in $runtimeFiles) {
    $fp = $f.FullName
    Step "delete runtime file $fp" { Remove-Item -Force $fp }
}

# 6. DNS cache
Step 'flush DNS cache (ipconfig /flushdns)' { ipconfig /flushdns | Out-Null }
$todo--  # flushdns is always listed, do not count it as a problem

if ($todo -eq 0) { Write-Host '[clean] nothing left over (besides the routine DNS flush).' }
elseif (-not $Apply) { Write-Host "[check] $todo item(s) to clean. Re-run with -Apply in an Administrator PowerShell." }
else { Write-Host '[done] re-run without -Apply to confirm everything is clean.' }
