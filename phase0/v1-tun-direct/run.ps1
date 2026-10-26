# V1: start sing-box TUN with everything going "direct". Run in an ADMINISTRATOR PowerShell.
# Stops with Ctrl+C. If anything goes wrong: phase0\v1-tun-direct\cleanup.ps1
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\common.ps1')
if (-not (Test-IsAdmin)) { Write-Host '[ERROR] Not Administrator. Right-click PowerShell > Run as administrator, then run this again.'; exit 1 }
Write-Host '[ok] running as Administrator'

$singbox = Join-Path $BinDir 'sing-box\sing-box.exe'
$config = Join-Path $PSScriptRoot 'config.json'
& (Join-Path $Phase0Dir 'snapshot.ps1') -Label 'v1-before' | Out-Null

& $singbox check -c $config
if ($LASTEXITCODE -ne 0) { Write-Host '[ERROR] sing-box check failed, not starting.'; exit 1 }

$log = Join-Path $LogDir ("v1-singbox-" + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
Write-Host "[start] sing-box (log: $log). Press Ctrl+C to stop."
& $singbox run -c $config 2>&1 | Tee-Object -FilePath $log
