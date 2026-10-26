# V7 step 2: remove the full-tunnel routes (0.0.0.0/1, 128.0.0.0/1) the official Pangolin client adds to its
# adapter when an Exit Node is active, then watch for 60 s whether the client puts them back.
# Without -Apply: only shows what it would remove (no admin needed).
# With -Apply   : ADMINISTRATOR PowerShell. Rollback: disconnect + reconnect in the Pangolin app
#                 (it re-adds them), or: phase0\v7-pangolin\drop-routes.ps1 -Restore
param(
    [string]$Interface = 'Pangolin',
    [switch]$Apply,
    [switch]$Restore,
    # Add (or with -Restore: remove) a lowest-priority 0.0.0.0/0 on the adapter. Windows refuses to send a
    # socket bound to an interface (sing-box bind_interface) when that interface has no route to the target.
    [switch]$LowPriorityDefault,
    [int]$LowMetric = 9999,
    [int]$WatchSeconds = 60
)
. (Join-Path $PSScriptRoot '..\common.ps1')
$ErrorActionPreference = 'Continue'
$prefixes = @('0.0.0.0/1', '128.0.0.0/1')
$a = Get-NetAdapter -Name $Interface -ErrorAction SilentlyContinue
if (-not $a) { Write-Host "[ERROR] adapter '$Interface' not found. Connect in the Pangolin app first."; exit 1 }

function Show-Routes {
    $r = @(Get-NetRoute -InterfaceIndex $a.ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.DestinationPrefix -in $prefixes })
    if ($r) { ($r | ForEach-Object { "$($_.DestinationPrefix) metric $($_.RouteMetric)" }) -join '; ' } else { '(none)' }
}

function Show-Low {
    $r = Get-NetRoute -InterfaceIndex $a.ifIndex -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue
    if ($r) { "0.0.0.0/0 metric $($r.RouteMetric)" } else { '(no 0.0.0.0/0)' }
}

if ($LowPriorityDefault) {
    $have = Get-NetRoute -InterfaceIndex $a.ifIndex -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue
    if ($Restore) { $cmd = "Remove-NetRoute -InterfaceIndex $($a.ifIndex) -DestinationPrefix 0.0.0.0/0 -Confirm:`$false" }
    else { $cmd = "New-NetRoute -InterfaceIndex $($a.ifIndex) -DestinationPrefix 0.0.0.0/0 -NextHop 0.0.0.0 -RouteMetric $LowMetric -PolicyStore ActiveStore" }
    Write-Host "[now] on ${Interface}: $(Show-Low); full-tunnel: $(Show-Routes)"
    if (-not $Apply) { Write-Host "[would] $cmd"; Write-Host '[check] re-run with -Apply in an Administrator PowerShell.'; exit 0 }
    if (-not (Test-IsAdmin)) { Write-Host '[ERROR] -Apply needs an Administrator PowerShell.'; exit 1 }
    if ($Restore -and $have) { Write-Host "[run] $cmd"; Remove-NetRoute -InterfaceIndex $a.ifIndex -DestinationPrefix '0.0.0.0/0' -Confirm:$false }
    elseif (-not $Restore -and -not $have) { Write-Host "[run] $cmd"; New-NetRoute -InterfaceIndex $a.ifIndex -DestinationPrefix '0.0.0.0/0' -NextHop 0.0.0.0 -RouteMetric $LowMetric -PolicyStore ActiveStore | Out-Null }
    Write-Host "[after] on ${Interface}: $(Show-Low)   route for 1.1.1.1 -> $((Find-NetRoute -RemoteIPAddress 1.1.1.1 | Select-Object -Last 1).InterfaceAlias)   exit IP: $((curl.exe -4 -s --max-time 8 https://api.ipify.org) -join '')"
    exit 0
}

if ($Restore) {
    if (-not (Test-IsAdmin)) { Write-Host '[ERROR] -Restore needs an Administrator PowerShell.'; exit 1 }
    foreach ($p in $prefixes) {
        if (-not (Get-NetRoute -InterfaceIndex $a.ifIndex -DestinationPrefix $p -ErrorAction SilentlyContinue)) {
            Write-Host "[run] New-NetRoute -InterfaceIndex $($a.ifIndex) -DestinationPrefix $p -NextHop 0.0.0.0 -RouteMetric 0 -PolicyStore ActiveStore"
            New-NetRoute -InterfaceIndex $a.ifIndex -DestinationPrefix $p -NextHop 0.0.0.0 -RouteMetric 0 -PolicyStore ActiveStore | Out-Null
        }
    }
    Write-Host "[now] $(Show-Routes)"; exit 0
}

Write-Host "[now] full-tunnel routes on ${Interface}: $(Show-Routes)"
if (-not $Apply) {
    foreach ($p in $prefixes) { Write-Host "[would] Remove-NetRoute -InterfaceIndex $($a.ifIndex) -DestinationPrefix $p -Confirm:`$false" }
    Write-Host '[check] re-run with -Apply in an Administrator PowerShell.'; exit 0
}
if (-not (Test-IsAdmin)) { Write-Host '[ERROR] -Apply needs an Administrator PowerShell.'; exit 1 }
& (Join-Path $Phase0Dir 'snapshot.ps1') -Label 'v7-before-drop' | Out-Null
foreach ($p in $prefixes) {
    if (Get-NetRoute -InterfaceIndex $a.ifIndex -DestinationPrefix $p -ErrorAction SilentlyContinue) {
        Write-Host "[run] Remove-NetRoute -InterfaceIndex $($a.ifIndex) -DestinationPrefix $p"
        Remove-NetRoute -InterfaceIndex $a.ifIndex -DestinationPrefix $p -Confirm:$false
    }
}
Write-Host "[after] $(Show-Routes)   exit IP: $((curl.exe -4 -s --max-time 8 https://api.ipify.org) -join '')"
Write-Host "[watch] checking every 5 s for $WatchSeconds s whether the Pangolin client re-adds them..."
for ($t = 5; $t -le $WatchSeconds; $t += 5) {
    Start-Sleep -Seconds 5
    Write-Host ("  +{0,3}s  {1}" -f $t, (Show-Routes))
}
Write-Host "[end] exit IP now: $((curl.exe -4 -s --max-time 8 https://api.ipify.org) -join '')"
