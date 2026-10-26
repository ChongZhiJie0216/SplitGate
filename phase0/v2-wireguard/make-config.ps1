# V2: turn a standard WireGuard .conf into a sing-box 1.14 config at phase0\runtime\v2-singbox.json.
# The private key is copied into that runtime file only (git-ignored, user-only ACL, deleted by cleanup).
# Nothing secret is printed.
# Usage: make-config.ps1 -Conf C:\path\home.conf [-VpnDomains ifconfig.me]
param(
    [Parameter(Mandatory = $true)][string]$Conf,
    [string[]]$VpnDomains = @('ifconfig.me')
)
. (Join-Path $PSScriptRoot '..\common.ps1')

if (-not (Test-Path $Conf)) { throw "WireGuard .conf not found: $Conf" }
$full = (Resolve-Path $Conf).Path
if ($full.StartsWith($RepoRoot, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Keep the .conf OUTSIDE the repository (it contains the private key): $full"
}

# Parse the INI-style .conf: section -> key -> list of values (keys may repeat, values may be comma lists)
$ini = @{}
$section = ''
foreach ($raw in Get-Content $full) {
    $line = ($raw -replace '[#;].*$', '').Trim()
    if ($line -eq '') { continue }
    if ($line -match '^\[(.+)\]$') { $section = $Matches[1].Trim().ToLower(); if (-not $ini[$section]) { $ini[$section] = @{} }; continue }
    if ($line -match '^([^=]+?)\s*=\s*(.+)$') {
        $k = $Matches[1].Trim().ToLower(); $v = $Matches[2].Trim()
        if (-not $ini[$section][$k]) { $ini[$section][$k] = @() }
        $ini[$section][$k] += @($v -split '\s*,\s*' | Where-Object { $_ })
    }
}
$if = $ini['interface']; $peer = $ini['peer']
if (-not $if -or -not $peer) { throw 'The .conf must have [Interface] and [Peer] sections.' }
foreach ($need in @(@($if, 'privatekey'), @($if, 'address'), @($peer, 'publickey'), @($peer, 'endpoint'))) {
    if (-not $need[0][$need[1]]) { throw "Missing '$($need[1])' in the .conf." }
}

# Endpoint: host:port or [v6]:port
$ep = $peer['endpoint'][0]
if ($ep -match '^\[(.+)\]:(\d+)$' -or $ep -match '^([^:]+):(\d+)$') { $epHost = $Matches[1]; $epPort = [int]$Matches[2] }
else { throw "Cannot parse Endpoint '$ep' (expected host:port)." }

# DNS line may mix servers and search domains; keep IP addresses only
$dns = @()
if ($if['dns']) { $dns = @($if['dns'] | Where-Object { $_ -as [ipaddress] }) }
if ($dns.Count -eq 0) { $dns = @('1.1.1.1'); Write-Host '[note] no DNS in .conf, vpn-dns falls back to 1.1.1.1 (through the tunnel)' }
$v4dns = @($dns | Where-Object { ([ipaddress]$_).AddressFamily -eq 'InterNetwork' })
if ($v4dns.Count -eq 0) { throw 'DNS in the .conf has no IPv4 address; this test is IPv4 only.' }

$wgPeer = [ordered]@{
    address     = $epHost
    port        = $epPort
    public_key  = $peer['publickey'][0]
    # Rules decide what enters the tunnel, so the tunnel must accept every destination.
    allowed_ips = @('0.0.0.0/0', '::/0')
}
if ($peer['presharedkey']) { $wgPeer.pre_shared_key = $peer['presharedkey'][0] }
if ($peer['persistentkeepalive']) { $wgPeer.persistent_keepalive_interval = [int]$peer['persistentkeepalive'][0] }

$wg = [ordered]@{
    type        = 'wireguard'
    tag         = 'vpn'
    address     = @($if['address'])
    private_key = $if['privatekey'][0]
    peers       = @($wgPeer)
}
if ($if['mtu']) { $wg.mtu = [int]$if['mtu'][0] }

$config = [ordered]@{
    log       = [ordered]@{ level = 'info'; timestamp = $true }
    dns       = [ordered]@{
        servers  = @(
            [ordered]@{ type = 'local'; tag = 'local' },
            [ordered]@{ type = 'udp'; tag = 'vpn-dns'; server = $v4dns[0]; detour = 'vpn' }
        )
        rules    = @([ordered]@{ domain_suffix = $VpnDomains; server = 'vpn-dns' })
        final    = 'local'
        strategy = 'ipv4_only'
    }
    inbounds  = @([ordered]@{
        type = 'tun'; tag = 'tun-in'; interface_name = 'SplitGate-TUN'
        address = @('172.19.0.1/30', 'fdfe:dcba:9876::1/126'); auto_route = $true; strict_route = $true
    })
    endpoints = @($wg)
    outbounds = @([ordered]@{ type = 'direct'; tag = 'direct' })
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

New-Item -ItemType Directory -Force $RuntimeDir | Out-Null
$out = Join-Path $RuntimeDir 'v2-singbox.json'
[IO.File]::WriteAllText($out, ($config | ConvertTo-Json -Depth 10), (New-Object Text.UTF8Encoding $false))
# Only the current user, Administrators and SYSTEM may read it
icacls $out /inheritance:r /grant:r "$($env:USERNAME):F" '*S-1-5-32-544:F' '*S-1-5-18:F' | Out-Null

Write-Host "[config] $out"
Write-Host "[config] peer endpoint $epHost`:$epPort, tunnel address $($if['address'] -join ', '), vpn-dns $($v4dns[0])"
Write-Host "[config] via VPN: $($VpnDomains -join ', '); everything else direct"
