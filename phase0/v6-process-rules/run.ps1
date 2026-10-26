# V6: route by PROGRAM instead of by domain. All traffic of the listed programs goes through the Cisco/AnyConnect
# exit (sing-box built-in OpenConnect, same as V3a); other programs stay direct even for the same website.
# Default rule: curl.exe (used by test.ps1). Add real programs with -VpnProcessPaths, e.g. PrismLauncher + its Java.
# Run in an ADMINISTRATOR PowerShell. You type the password. Stops with Ctrl+C.
# If anything goes wrong: phase0\v6-process-rules\cleanup.ps1
param(
    [string[]]$VpnProcesses = @('curl.exe'),
    [string[]]$VpnProcessPaths = @(),
    [string]$AuthGroup = 'Master Network',
    [ValidateSet('trace', 'debug', 'info', 'warn', 'error')][string]$LogLevel = 'info',
    [switch]$CheckOnly
)
$v3a = Join-Path $PSScriptRoot '..\v3a-singbox-openconnect\run.ps1'
# ifconfig.me stays as the only domain rule; test.ps1 uses api.ipify.org, which is NOT a domain rule.
$splat = @{ VpnDomains = @('ifconfig.me'); AuthGroup = $AuthGroup; NoUdp = $true; LogLevel = $LogLevel }
if ($VpnProcesses) { $splat.VpnProcesses = $VpnProcesses }
if ($VpnProcessPaths) { $splat.VpnProcessPaths = $VpnProcessPaths }
if ($CheckOnly) { $splat.CheckOnly = $true }
& $v3a @splat
exit $LASTEXITCODE
