# V1 checks, run in a SECOND PowerShell while run.ps1 is running. Read-only.
#   -Kill : also force-kill sing-box and measure how long until the network works again (needs admin).
param([switch]$Kill)
. (Join-Path $PSScriptRoot '..\common.ps1')
$ErrorActionPreference = 'Continue'

function Test-Web {
    # Default address family, the way a browser would connect.
    $code = curl.exe -s -o NUL -w '%{http_code}' --max-time 2 https://www.microsoft.com
    return ($code -match '^[23]')
}

$running = @(Get-Process sing-box -ErrorAction SilentlyContinue)
$adapters = @(Get-OurAdapters)
if ($running.Count -eq 0 -or $adapters.Count -eq 0) {
    Write-Host "[ERROR] sing-box running: $($running.Count -gt 0); SplitGate adapter present: $($adapters.Count -gt 0)."
    Write-Host '        Start phase0\v1-tun-direct\run.ps1 in an Administrator PowerShell first. Test NOT run.'
    exit 1
}

Write-Host '--- adapter ---'
$adapters | Format-Table Name, Status, InterfaceDescription -AutoSize | Out-String | Write-Host
Write-Host '--- default routes ---'
Get-DefaultRoutes | Format-Table -AutoSize | Out-String | Write-Host
Write-Host '--- routes on SplitGate adapter ---'
Get-NetRoute -InterfaceAlias $OurAdapterPattern -ErrorAction SilentlyContinue | Select-Object DestinationPrefix, NextHop, RouteMetric | Format-Table -AutoSize | Out-String | Write-Host
Write-Host '--- exit IP (should be the local exit) ---'
Write-Host ('IPv4: ' + (curl.exe -4 -s --max-time 8 https://ifconfig.me))
Write-Host ('IPv6: ' + (curl.exe -6 -s --max-time 8 https://ifconfig.me))
Write-Host '--- DNS ---'
Resolve-DnsName www.xbox.com -Type A -DnsOnly -ErrorAction SilentlyContinue | Select-Object -First 3 Name, IPAddress | Format-Table -AutoSize | Out-String | Write-Host
Write-Host "--- web (www.microsoft.com, default family): $(Test-Web) ---"
Write-Host "--- web (www.google.com): $((curl.exe -s -o NUL -w '%{http_code}' --max-time 5 https://www.google.com) -match '^[23]') ---"

if ($Kill) {
    if (-not (Test-IsAdmin)) { Write-Host '[ERROR] -Kill needs admin.'; exit 1 }
    Write-Host '[kill] Stop-Process -Force sing-box'
    $t = [Diagnostics.Stopwatch]::StartNew()
    Get-Process sing-box -ErrorAction SilentlyContinue | Stop-Process -Force
    while ($t.Elapsed.TotalSeconds -lt 30) {
        if (Test-Web) { break }
        Start-Sleep -Milliseconds 250
    }
    Write-Host ("[kill] network back after {0:N1} s (target: under 5 s)" -f $t.Elapsed.TotalSeconds)
    Write-Host '--- leftover check ---'
    & (Join-Path $Phase0Dir 'cleanup-all.ps1')
}
