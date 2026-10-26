# V5: route only the domains the school Fortinet intercepts (Xbox Live auth, minecraft.net, Steam) through the
# Cisco/AnyConnect exit (sing-box built-in OpenConnect, same as V3a). Everything else stays direct.
# Run in an ADMINISTRATOR PowerShell. You type the password. Stops with Ctrl+C.
# Then, in a second PowerShell: phase0\v5-block-probe\probe.ps1 -ViaTun
# If anything goes wrong: phase0\v5-block-probe\cleanup.ps1
param(
    [string[]]$VpnDomains = @(
        'ifconfig.me',
        'xboxlive.com', 'xbox.com',
        'minecraft.net',
        'steampowered.com', 'steamcommunity.com', 'steamserver.net', 'steamstatic.com', 'steamcdn-a.akamaihd.net'
    ),
    [string]$AuthGroup = 'Master Network',
    [string]$BindInterface = '',
    [ValidateSet('trace', 'debug', 'info', 'warn', 'error')][string]$LogLevel = 'info',
    [switch]$CheckOnly
)
$v3a = Join-Path $PSScriptRoot '..\v3a-singbox-openconnect\run.ps1'
$splat = @{ VpnDomains = $VpnDomains; AuthGroup = $AuthGroup; NoUdp = $true; LogLevel = $LogLevel }
if ($BindInterface) { $splat.BindInterface = $BindInterface }
if ($CheckOnly) { $splat.CheckOnly = $true }
& $v3a @splat
exit $LASTEXITCODE
