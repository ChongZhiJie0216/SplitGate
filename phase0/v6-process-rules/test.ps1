# V6 test (read-only): the same URL (api.ipify.org, not a domain rule) from two programs.
#   curl.exe        (process rule)  -> expect the VPN exit IP
#   powershell.exe  (no rule)       -> expect the local exit IP
param([string]$HomeIp = '202.186.213.216')
. (Join-Path $PSScriptRoot '..\common.ps1')
$ErrorActionPreference = 'Continue'
if (-not (Get-OurProcesses)) { Write-Host '[ERROR] sing-box is not running. Start phase0\v6-process-rules\run.ps1 first. Test NOT run.'; exit 1 }
$log = Get-ChildItem (Join-Path $LogDir 'v3a-singbox-*.log') -ErrorAction SilentlyContinue | Sort-Object LastWriteTime | Select-Object -Last 1
if (-not $log -or -not (Select-String -Path $log.FullName -Pattern 'tunnel established' -Quiet)) {
    Write-Host '[ERROR] VPN not connected yet (no "tunnel established" in the sing-box log). Wait for it, then run again. Test NOT run.'; exit 1
}

$viaCurl = (curl.exe -s --max-time 8 https://api.ipify.org) -join ''
try { $viaPs = (Invoke-RestMethod -UseBasicParsing -TimeoutSec 8 https://api.ipify.org) } catch { $viaPs = '' }
Write-Host "curl.exe       (process rule, expect VPN)  : $viaCurl"
Write-Host "powershell.exe (no rule,      expect LOCAL): $viaPs"
if ($viaCurl -eq $HomeIp) { Write-Host '[PASS] curl.exe went through the VPN' } else { Write-Host "[FAIL] curl.exe exit is not $HomeIp" }
if ($viaPs -and $viaPs -ne $HomeIp) { Write-Host '[PASS] powershell.exe stayed direct' } else { Write-Host '[FAIL] powershell.exe exit is empty or the VPN IP' }

Write-Host '--- sing-box log: which outbound each program used for api.ipify.org ---'
Start-Sleep -Milliseconds 500
$ips = @([Net.Dns]::GetHostAddresses('api.ipify.org') | ForEach-Object { $_.IPAddressToString })
Select-String -Path $log.FullName -Pattern 'outbound connection to (\S+):443' |
    Where-Object { $ips -contains $_.Matches[0].Groups[1].Value } | Select-Object -Last 4 | ForEach-Object { $_.Line }

Write-Host '--- leftover check ---'
& (Join-Path $Phase0Dir 'cleanup-all.ps1')
