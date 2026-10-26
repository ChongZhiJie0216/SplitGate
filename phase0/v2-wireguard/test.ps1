# V2 checks, run in a SECOND Administrator PowerShell while run.ps1 is running.
#   -HomeIp <ip> : expected public IP of the WireGuard exit (to print PASS/FAIL)
#   -Kill        : also force-kill sing-box and measure how long until the network works again
param([string]$HomeIp = '', [switch]$Kill)
. (Join-Path $PSScriptRoot '..\common.ps1')
$ErrorActionPreference = 'Continue'

function Test-Web {
    $code = curl.exe -s -o NUL -w '%{http_code}' --max-time 2 https://www.microsoft.com
    return ($code -match '^[23]')
}

if (@(Get-Process sing-box -ErrorAction SilentlyContinue).Count -eq 0) {
    Write-Host '[ERROR] sing-box is not running. Start phase0\v2-wireguard\run.ps1 first. Test NOT run.'
    exit 1
}

$local = (curl.exe -4 -s --max-time 10 https://api.ipify.org)
$viaVpn = (curl.exe -4 -s --max-time 10 https://ifconfig.me)
Write-Host "api.ipify.org (not in rules, expect LOCAL): $local"
Write-Host "ifconfig.me   (in rules,     expect HOME) : $viaVpn"
if ($viaVpn -and $local -and $viaVpn -ne $local) { Write-Host '[PASS] the two exits are different' } else { Write-Host '[FAIL] the two exits are the same or empty' }
if ($HomeIp) { if ($viaVpn -eq $HomeIp) { Write-Host '[PASS] ifconfig.me exit = home IP' } else { Write-Host "[FAIL] ifconfig.me exit is not $HomeIp" } }

Write-Host '--- DNS for ifconfig.me (sing-box log should show vpn-dns for it) ---'
Resolve-DnsName ifconfig.me -Type A -DnsOnly -ErrorAction SilentlyContinue | Select-Object Name, IPAddress | Format-Table -AutoSize | Out-String | Write-Host
Write-Host "--- web (www.microsoft.com): $(Test-Web) ---"

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
