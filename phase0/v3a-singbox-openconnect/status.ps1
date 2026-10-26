# Read-only: print the OpenConnect endpoint status / pending login form of the running V3a sing-box.
# Works from a normal (non-admin) PowerShell while run.ps1 is running.
. (Join-Path $PSScriptRoot '..\common.ps1')
$apiEnv = Join-Path $RuntimeDir 'v3a-api.env'
if (-not (Test-Path $apiEnv)) { Write-Host '[ERROR] run.ps1 is not running (no phase0\runtime\v3a-api.env).'; exit 1 }
foreach ($line in Get-Content $apiEnv) {
    if ($line -match '^(BOX_API_URL|BOX_API_SECRET)=(.+)$') { Set-Item -Path "env:$($Matches[1])" -Value $Matches[2] }
}
& (Join-Path $BinDir 'sing-box\sing-box.exe') api openconnect status 2>&1 | ForEach-Object { "$_" }
