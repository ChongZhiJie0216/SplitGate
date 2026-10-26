# V2: sing-box TUN + WireGuard endpoint; only ifconfig.me goes through the tunnel.
# Run in an ADMINISTRATOR PowerShell. Stops with Ctrl+C; the runtime config (with the key) is deleted on exit.
# If anything goes wrong: phase0\v2-wireguard\cleanup.ps1
param([string]$Conf = (Join-Path $env:USERPROFILE 'splitgate-secrets\home.conf'))
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\common.ps1')
if (-not (Test-IsAdmin)) { Write-Host '[ERROR] Not Administrator. Right-click PowerShell > Run as administrator, then run this again.'; exit 1 }
Write-Host '[ok] running as Administrator'

$singbox = Join-Path $BinDir 'sing-box\sing-box.exe'
$config = Join-Path $RuntimeDir 'v2-singbox.json'
& (Join-Path $Phase0Dir 'snapshot.ps1') -Label 'v2-before' | Out-Null
& (Join-Path $PSScriptRoot 'make-config.ps1') -Conf $Conf

try {
    & $singbox check -c $config
    if ($LASTEXITCODE -ne 0) { Write-Host '[ERROR] sing-box check failed, not starting.'; exit 1 }

    $log = Join-Path $LogDir ("v2-singbox-" + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
    Write-Host "[start] sing-box (log: $log). Press Ctrl+C to stop."
    # cmd.exe merges stderr; see v1-tun-direct\run.ps1 for why
    $ErrorActionPreference = 'Continue'
    cmd.exe /c "`"$singbox`" run --disable-color -c `"$config`" 2>&1" | Tee-Object -FilePath $log
    Write-Host "[exit] sing-box exited with code $LASTEXITCODE"
} finally {
    Remove-Item -Force $config -ErrorAction SilentlyContinue
    Write-Host '[cleanup] runtime config deleted'
}
