# V7 observe (READ-ONLY, no admin needed): what did the Pangolin client change compared to the baseline?
# -Interface = the adapter name the client created (if unknown, all wintun / WireGuard adapters are listed).
#   adapter, its addresses and DNS, every route on it, default routes, NRPT rules, and the public exit IP.
param([string]$Interface = '')
. (Join-Path $PSScriptRoot '..\common.ps1')
$ErrorActionPreference = 'Continue'

Write-Host "--- adapter '$Interface' ---"
$a = if ($Interface) { Get-NetAdapter -IncludeHidden -Name $Interface -ErrorAction SilentlyContinue } else { $null }
if (-not $a) { Write-Host '[none] adapter not given or not found; wintun / WireGuard adapters:'; Get-NetAdapter -IncludeHidden | Where-Object { $_.InterfaceDescription -match 'wintun|WireGuard' } | Format-Table Name, InterfaceDescription, Status -AutoSize | Out-String -Width 200 }
else {
    $a | Format-Table Name, InterfaceDescription, Status, ifIndex -AutoSize | Out-String -Width 200
    Get-NetIPAddress -InterfaceIndex $a.ifIndex -ErrorAction SilentlyContinue | Format-Table IPAddress, PrefixLength, AddressFamily -AutoSize | Out-String
    Write-Host "DNS on ${Interface}: $((Get-DnsClientServerAddress -InterfaceIndex $a.ifIndex -ErrorAction SilentlyContinue).ServerAddresses -join ', ')"
    Write-Host "--- routes on $Interface ---"
    Get-NetRoute -InterfaceIndex $a.ifIndex -ErrorAction SilentlyContinue | Sort-Object DestinationPrefix | Format-Table DestinationPrefix, NextHop, RouteMetric -AutoSize | Out-String
}
Write-Host '--- default routes (baseline: Wi-Fi + Ethernet only) ---'
Get-DefaultRoutes | Format-Table -AutoSize | Out-String
Write-Host '--- DNS servers per adapter (static ones changed vs baseline?) ---'
$base = $null; if (Test-Path $BaselinePath) { $base = Get-Content $BaselinePath -Raw | ConvertFrom-Json }
foreach ($s in Get-StaticDnsState | Where-Object { $_.NameServer }) {
    $was = if ($base) { ($base.StaticDns | Where-Object { $_.Guid -eq $s.Guid }).NameServer } else { '?' }
    Write-Host ("{0,-25} now='{1}' baseline='{2}'" -f $s.Alias, $s.NameServer, $was)
}
Write-Host "--- NRPT rules: $(@(Get-DnsClientNrptRule -ErrorAction SilentlyContinue).Count) (baseline 0) ---"
Write-Host "--- public exit IP (api.ipify.org): $((curl.exe -s --max-time 8 https://api.ipify.org) -join '') ---"
