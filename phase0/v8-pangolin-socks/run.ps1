# V8: Pangolin exit WITHOUT Exit Node. sing-box TUN; rule traffic (default: ifconfig.me) goes to the
# SOCKS5 proxy on the home VPN-Server VM, reached through the official Pangolin client's adapter
# (socks outbound with bind_interface). Everything else direct. No routes are added or removed.
# Before: Pangolin admin -> Exit Node OFF; Pangolin app connected (site only);
#         proxy installed on the VM (server\install-socks.sh).
# Run in an ADMINISTRATOR PowerShell. The proxy password is asked at a hidden prompt and only
# goes to sing-box through stdin (never a file, argument or log). Stops with Ctrl+C.
# If anything goes wrong: phase0\v8-pangolin-socks\cleanup.ps1 (does NOT touch the Pangolin app's adapter)
param(
    [Parameter(Mandatory = $true)][string]$ProxyIp,
    [int]$ProxyPort = 1080,
    [string]$ProxyUser = 'splitgate',
    [string]$Interface = 'Pangolin',
    [string[]]$VpnDomains = @('ifconfig.me'),
    # DNS for rule domains, over TCP through the proxy (SOCKS5 + Pangolin: TCP is the safe path).
    [string]$VpnDns = '9.9.9.9',
    # DNS for everything else, direct. Not 'local': Windows lists Pangolin's DNS (100.96.128.1) first.
    [string]$LocalDns = '1.1.1.1',
    [ValidateSet('trace', 'debug', 'info', 'warn', 'error')][string]$LogLevel = 'info',
    [switch]$CheckOnly
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\common.ps1')
. (Join-Path $Phase0Dir 'stdin-run.ps1')
$VpnDomains = @($VpnDomains | ForEach-Object { "$_" -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
if (-not $CheckOnly) {
    if (-not (Test-IsAdmin)) { Write-Host '[ERROR] Not Administrator. Right-click PowerShell > Run as administrator, then run this again.'; exit 1 }
    if (-not (Get-NetAdapter -Name $Interface -ErrorAction SilentlyContinue)) { Write-Host "[ERROR] adapter '$Interface' not found. Connect in the Pangolin app first."; exit 1 }
    if (Get-OurProcesses) { Write-Host '[ERROR] sing-box is already running. Stop it first.'; exit 1 }
    # bind_interface needs a route on that adapter (phase 0 V7): the proxy must be inside a Pangolin site resource.
    $r = Find-NetRoute -RemoteIPAddress $ProxyIp -ErrorAction SilentlyContinue | Where-Object { $_.InterfaceAlias } | Select-Object -First 1
    if (-not $r -or $r.InterfaceAlias -ne $Interface) {
        Write-Host "[ERROR] $ProxyIp is not routed through '$Interface' (now: $($r.InterfaceAlias)). Add it (or its subnet) as a site resource in Pangolin."
        exit 1
    }
    $full = @(Get-NetRoute -InterfaceAlias $Interface -ErrorAction SilentlyContinue | Where-Object { $_.DestinationPrefix -in @('0.0.0.0/1', '128.0.0.0/1', '0.0.0.0/0') })
    if ($full.Count -gt 0) { Write-Host "[warn] '$Interface' has full-tunnel routes ($($full.DestinationPrefix -join ', ')): Exit Node is still on. Turn it off in the Pangolin admin for this test." }
}

$pw = 'check-only-placeholder'
if (-not $CheckOnly) {
    $ss = Read-Host -AsSecureString "Proxy password for $ProxyUser@${ProxyIp}:$ProxyPort"
    $bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($ss)
    try { $pw = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr) } finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr) }
}

if ($LocalDns) { $localDnsServer = [ordered]@{ type = 'udp'; tag = 'local'; server = $LocalDns } } else { $localDnsServer = [ordered]@{ type = 'local'; tag = 'local' } }
$config = [ordered]@{
    log       = [ordered]@{ level = $LogLevel; timestamp = $true }
    dns       = [ordered]@{
        servers  = @($localDnsServer, [ordered]@{ type = 'tcp'; tag = 'vpn-dns'; server = $VpnDns; detour = 'vpn' })
        rules    = @([ordered]@{ domain_suffix = $VpnDomains; server = 'vpn-dns' })
        final    = 'local'
        strategy = 'ipv4_only'
    }
    inbounds  = @([ordered]@{
        type = 'tun'; tag = 'tun-in'; interface_name = 'SplitGate-TUN'
        address = @('172.19.0.1/30', 'fdfe:dcba:9876::1/126'); auto_route = $true; strict_route = $true
    })
    outbounds = @(
        [ordered]@{ type = 'direct'; tag = 'direct' },
        [ordered]@{ type = 'socks'; tag = 'vpn'; server = $ProxyIp; server_port = $ProxyPort; version = '5'; username = $ProxyUser; password = $pw; bind_interface = $Interface }
    )
    route     = [ordered]@{
        rules                   = @(
            [ordered]@{ action = 'sniff' },
            [ordered]@{ protocol = 'dns'; action = 'hijack-dns' },
            [ordered]@{ ip_version = 6; action = 'reject' },
            [ordered]@{ domain_suffix = $VpnDomains; action = 'route'; outbound = 'vpn' }
        )
        final                   = 'direct'
        auto_detect_interface   = $true
        default_domain_resolver = [ordered]@{ server = 'local' }
    }
}
$json = $config | ConvertTo-Json -Depth 10
$pw = $null; $config = $null
Write-Host "[config] via SOCKS5 ${ProxyIp}:$ProxyPort on $Interface : $($VpnDomains -join ', '); vpn-dns tcp $VpnDns through it"
$ErrorActionPreference = 'Continue'
$rc = Invoke-SingBoxStdin -Mode check -ConfigJson $json
if ($rc -ne 0) { Write-Host '[ERROR] sing-box check failed, not starting.'; exit 1 }
if ($CheckOnly) { Write-Host '[check-only] config OK'; exit 0 }

& (Join-Path $Phase0Dir 'snapshot.ps1') -Label 'v8-before' | Out-Null
New-Item -ItemType Directory -Force $LogDir | Out-Null
$log = Join-Path $LogDir ("v8-socks-" + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
Write-Host "[start] sing-box (log: $log). Press Ctrl+C to stop."
Write-Host '        Watch the Pangolin app log at the same time: does it disconnect when sing-box starts?'
$rc = Invoke-SingBoxStdin -Mode run -ConfigJson $json -LogFile $log
Write-Host "[exit] sing-box exited with code $rc"
