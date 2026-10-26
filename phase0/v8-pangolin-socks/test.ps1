# V8 checks, run in a SECOND Administrator PowerShell while run.ps1 is running. READ-ONLY.
#   -HomeIp <ip>       : expected public IP of home (ifconfig.me must show it)
#   -HomeTarget ip:port: a home device to reach while SplitGate-style split is on, e.g. the PVE web UI
#                        (<pve-ip>:8006). It is not in any rule: it must go through Pangolin's own routes.
param([string]$HomeIp = '', [string]$HomeTarget = '')
. (Join-Path $PSScriptRoot '..\common.ps1')
$ErrorActionPreference = 'Continue'
& (Join-Path $Phase0Dir 'split-test.ps1') -HomeIp $HomeIp
if ($HomeTarget) {
    $h, $p = $HomeTarget -split ':'
    $c = New-Object Net.Sockets.TcpClient
    $ok = $false
    try { $ok = $c.ConnectAsync($h, [int]$p).Wait(3000) -and $c.Connected } catch { } finally { $c.Close() }
    $route = Find-NetRoute -RemoteIPAddress $h -ErrorAction SilentlyContinue | Where-Object { $_.InterfaceAlias } | Select-Object -First 1
    Write-Host "home device $HomeTarget via '$($route.InterfaceAlias)': $(if ($ok) { '[PASS] reachable' } else { '[FAIL] not reachable' })"
}
