# V4: same as V3a (sing-box built-in OpenConnect), but the VPN's own packets are forced out of one adapter
# (default: Wi-Fi = phone hotspot) while all other traffic keeps using the default adapter (Ethernet).
# Run in an ADMINISTRATOR PowerShell. You type the password. Stops with Ctrl+C.
# If anything goes wrong: phase0\v4-bind-interface\cleanup.ps1
param(
    [string]$BindInterface = 'Wi-Fi',
    [string]$AuthGroup = 'Master Network',
    # Default here: try DTLS (UDP). The hotspot may allow it even though the school network does not.
    [switch]$NoUdp,
    [ValidateSet('trace', 'debug', 'info', 'warn', 'error')][string]$LogLevel = 'info',
    [switch]$CheckOnly
)
$v3a = Join-Path $PSScriptRoot '..\v3a-singbox-openconnect\run.ps1'
$splat = @{ BindInterface = $BindInterface; AuthGroup = $AuthGroup; LogLevel = $LogLevel }
if ($NoUdp) { $splat.NoUdp = $true }
if ($CheckOnly) { $splat.CheckOnly = $true }
& $v3a @splat
exit $LASTEXITCODE
