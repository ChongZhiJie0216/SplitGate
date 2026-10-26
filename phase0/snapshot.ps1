# Save a READ-ONLY snapshot of routes, adapters and DNS to phase0\logs\snapshot-<time>\.
# The first run (or -Baseline) also writes phase0\logs\baseline.json, which cleanup-all.ps1 compares against.
# Run BEFORE and AFTER every network test. Does not change anything.
# Usage: powershell -ExecutionPolicy Bypass -File phase0\snapshot.ps1 [-Label before-v1] [-Baseline]
param([string]$Label = 'snapshot', [switch]$Baseline)
. (Join-Path $PSScriptRoot 'common.ps1')

$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$dir = Join-Path $LogDir "$stamp-$Label"
New-Item -ItemType Directory -Force $dir | Out-Null

route print | Out-File (Join-Path $dir 'route-print.txt') -Encoding utf8
ipconfig /all | Out-File (Join-Path $dir 'ipconfig-all.txt') -Encoding utf8
Get-NetRoute -ErrorAction SilentlyContinue | Select-Object DestinationPrefix, NextHop, InterfaceAlias, InterfaceIndex, RouteMetric, Protocol |
    ConvertTo-Json -Depth 3 | Out-File (Join-Path $dir 'routes.json') -Encoding utf8
Get-NetAdapter -IncludeHidden -ErrorAction SilentlyContinue | Select-Object Name, InterfaceDescription, InterfaceGuid, ifIndex, Status, MacAddress |
    ConvertTo-Json -Depth 3 | Out-File (Join-Path $dir 'adapters.json') -Encoding utf8
Get-DnsClientServerAddress -ErrorAction SilentlyContinue | Select-Object InterfaceAlias, InterfaceIndex, AddressFamily, ServerAddresses |
    ConvertTo-Json -Depth 3 | Out-File (Join-Path $dir 'dns-servers.json') -Encoding utf8
Get-DnsClientNrptRule -ErrorAction SilentlyContinue | Select-Object Name, Namespace, NameServers, Comment |
    ConvertTo-Json -Depth 3 | Out-File (Join-Path $dir 'nrpt.json') -Encoding utf8
Get-OurProcesses | Format-Table -AutoSize | Out-String | Out-File (Join-Path $dir 'processes.txt') -Encoding utf8

$state = [pscustomobject]@{
    Time          = (Get-Date).ToString('s')
    DefaultRoutes = @(Get-DefaultRoutes)
    StaticDns     = @(Get-StaticDnsState)
    Adapters      = @(Get-NetAdapter -IncludeHidden -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Name)
    NrptRules     = @(Get-DnsClientNrptRule -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Name)
}
$state | ConvertTo-Json -Depth 4 | Out-File (Join-Path $dir 'state.json') -Encoding utf8

if ($Baseline -or -not (Test-Path $BaselinePath)) {
    $state | ConvertTo-Json -Depth 4 | Out-File $BaselinePath -Encoding utf8
    Write-Host "[baseline] written: $BaselinePath"
}

Write-Host "[snapshot] $dir"
Write-Host '--- default routes ---'
$state.DefaultRoutes | Format-Table -AutoSize | Out-String | Write-Host
Write-Host '--- SplitGate adapters ---'
$ours = @(Get-OurAdapters)
if ($ours.Count -eq 0) { Write-Host '(none)' } else { $ours | Format-Table Name, Status, InterfaceDescription -AutoSize | Out-String | Write-Host }
