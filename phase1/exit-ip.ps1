# Phase 1 acceptance: "while connected, non-rule traffic still leaves through the school network". READ-ONLY.
# Run once DISCONNECTED (saves the public IP to phase1\logs\exit-ip-direct.txt), then once
# CONNECTED: the IP must be the same. The services used here are not in any rule set, so
# while connected they show where non-rule traffic goes.
# Usage: powershell -ExecutionPolicy Bypass -File phase1\exit-ip.ps1
. (Join-Path $PSScriptRoot 'common.ps1')

function Get-PublicIp {
    foreach ($u in @('https://api.ipify.org', 'https://checkip.amazonaws.com')) {
        try { $ip = (Invoke-WebRequest -Uri $u -UseBasicParsing -TimeoutSec 10).Content.Trim(); if ($ip) { return $ip } } catch { Write-Host "[warn] $u : $($_.Exception.Message)" }
    }
    return $null
}

[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$saved = Join-Path $Phase1LogDir 'exit-ip-direct.txt'
$ip = Get-PublicIp
if (-not $ip) { Write-Host '[ERROR] could not get the public IP.'; exit 1 }

if (-not (Test-Connected)) {
    New-Item -ItemType Directory -Force $Phase1LogDir | Out-Null
    Set-Content -Path $saved -Value $ip -Encoding ascii
    Write-Host "[direct] public IP $ip saved. Now connect SplitGate and run this again."
    exit 0
}
if (-not (Test-Path $saved)) { Write-Host "[connected] public IP $ip. No disconnected reference yet: disconnect, run once, reconnect, run again."; exit 1 }
$ref = (Get-Content $saved -Raw).Trim()
if ($ip -eq $ref) { Write-Host "[PASS] connected, non-rule traffic leaves as $ip (same as disconnected)."; exit 0 }
Write-Host "[FAIL] connected public IP $ip differs from disconnected $ref : non-rule traffic goes somewhere else."
exit 1
