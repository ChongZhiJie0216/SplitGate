# V9: TWO VPN exits at the same time in ONE sing-box (exit A and exit B, each Cisco or WireGuard).
# Only the test domains (and the networks you give) go through each exit; everything else stays direct.
# Run in an ADMINISTRATOR PowerShell. Cisco passwords are typed here; WireGuard keys are read from your
# own .conf file. Both are handed to sing-box on STDIN only - never to disk, the command line or the log.
# Stops with Ctrl+C. If anything goes wrong: phase0\v9-two-exits\cleanup.ps1
# Example (Cisco + WireGuard):
#   .\phase0\v9-two-exits\run.ps1 -AKind cisco -AServer https://<server>:8443 -AUser <user> -NoUdp `
#       -BKind wireguard -BConf D:\path\home.conf -BCidrs 192.168.10.0/24
param(
    [Parameter(Mandatory = $true)][ValidateSet('cisco', 'wireguard')][string]$AKind,
    [string]$AServer = '', [string]$AUser = '', [string]$AGroup = '', [string]$ABind = '', [string]$AConf = '',
    [string[]]$ADomains = @('ifconfig.me'),
    # Networks that go through exit A, e.g. its home LAN 192.168.1.0/24 (must not overlap B's)
    [string[]]$ACidrs = @(),
    [Parameter(Mandatory = $true)][ValidateSet('cisco', 'wireguard')][string]$BKind,
    [string]$BServer = '', [string]$BUser = '', [string]$BGroup = '', [string]$BBind = '', [string]$BConf = '',
    [string[]]$BDomains = @('api.ipify.org'),
    [string[]]$BCidrs = @(),
    # Cisco exits: TLS (TCP) only, no DTLS (school networks block it, phase 0 V3a)
    [switch]$NoUdp,
    [ValidateSet('trace', 'debug', 'info', 'warn', 'error')][string]$LogLevel = 'info',
    # Developer check: dummy secrets, "sing-box check" only. No admin needed, nothing connects.
    [switch]$CheckOnly
)
$ErrorActionPreference = 'Stop'
function Split-List($list) { @($list | ForEach-Object { "$_" -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ }) }
$ADomains = Split-List $ADomains; $BDomains = Split-List $BDomains; $ACidrs = Split-List $ACidrs; $BCidrs = Split-List $BCidrs
. (Join-Path $PSScriptRoot '..\common.ps1')
. (Join-Path $Phase0Dir 'stdin-run.ps1')
if (-not $CheckOnly) {
    if (-not (Test-IsAdmin)) { Write-Host '[ERROR] Not Administrator. Right-click PowerShell > Run as administrator, then run this again.'; exit 1 }
    Write-Host '[ok] running as Administrator'
}

function Get-HostOf([string]$s) {
    $h = $s -replace '^[a-zA-Z]+://', ''
    $h = ($h -split '/')[0]
    if ($h -match '^\[(.+)\]') { return $Matches[1] }
    return ($h -split ':')[0]
}

# One exit -> @{ Endpoint; Dns; ServerHost; Label }. Tags: vpn-a / vpn-b, DNS a-dns / b-dns.
function New-Exit([string]$Name, [string]$Kind, [string]$Server, [string]$User, [string]$Group, [string]$Bind, [string]$Conf) {
    $tag = "vpn-$Name"
    if ($Kind -eq 'cisco') {
        if (-not $Server) { Write-Host "[ERROR] exit $($Name.ToUpper()): give -$($Name.ToUpper())Server"; exit 1 }
        if ($CheckOnly) { $User = 'test-user'; $pw = 'dummy-password' }
        else {
            if (-not $User) { $User = Read-Host "Exit $($Name.ToUpper()) VPN username" }
            $secure = Read-Host "Exit $($Name.ToUpper()) VPN password for $User (not shown)" -AsSecureString
            $bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
            try { $pw = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr) } finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr) }
        }
        $url = if ($Server -match '^https?://') { $Server.TrimEnd('/') } else { "https://$Server" }
        $ep = [ordered]@{ type = 'openconnect'; tag = $tag; server = $url; username = $User; password = $pw }
        if ($Group) { $ep.auth_group = $Group }
        if ($NoUdp) { $ep.no_udp = $true }
        if ($Bind) {
            if (-not $CheckOnly -and -not (Get-NetAdapter -Name $Bind -ErrorAction SilentlyContinue)) { Write-Host "[ERROR] Adapter '$Bind' not found (see Get-NetAdapter)."; exit 1 }
            $ep.bind_interface = $Bind
        }
        $dns = [ordered]@{ type = 'openconnect'; tag = "$Name-dns"; endpoint = $tag; accept_default_resolvers = $true }
        return @{ Endpoint = $ep; Dns = $dns; ServerHost = (Get-HostOf $Server); Label = "Cisco $url user $User" }
    }
    # WireGuard: read your own .conf (one [Peer]); the private key only goes to sing-box's stdin.
    if ($CheckOnly -and -not $Conf) {
        $text = "[Interface]`nPrivateKey = $('A' * 43)=`nAddress = 10.8.0.2/32`n[Peer]`nPublicKey = $('A' * 43)=`nEndpoint = wg.example.com:51820"
    } else {
        if (-not $Conf -or -not (Test-Path $Conf)) { Write-Host "[ERROR] exit $($Name.ToUpper()): give -$($Name.ToUpper())Conf <path to .conf>"; exit 1 }
        $text = Get-Content -Raw -LiteralPath $Conf
    }
    $section = ''; $v = @{ address = @(); dns = @() }
    foreach ($raw in ($text -split "`r?`n")) {
        $line = ($raw -split '#')[0].Trim()
        if (-not $line) { continue }
        if ($line -match '^\[(.+)\]$') { $section = $Matches[1].Trim().ToLower(); continue }
        $kv = $line -split '=', 2
        if ($kv.Count -ne 2) { continue }
        $k = "$section.$($kv[0].Trim().ToLower())"; $val = $kv[1].Trim()
        switch ($k) {
            'interface.privatekey' { $v.key = $val }
            'interface.address' { $v.address += @($val -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ }) }
            'interface.dns' { $v.dns += @($val -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ -match '^\d+\.\d+\.\d+\.\d+$' }) }
            'interface.mtu' { $v.mtu = [int]$val }
            'peer.publickey' { $v.pub = $val }
            'peer.presharedkey' { $v.psk = $val }
            'peer.endpoint' { $v.endpoint = $val }
            'peer.persistentkeepalive' { $v.keepalive = [int]$val }
        }
    }
    $text = $null
    if (-not $v.key -or -not $v.pub -or -not $v.endpoint -or -not $v.address) { Write-Host "[ERROR] exit $($Name.ToUpper()): .conf is missing PrivateKey / Address / PublicKey / Endpoint"; exit 1 }
    $i = $v.endpoint.LastIndexOf(':')
    $wgHost = Get-HostOf $v.endpoint; $port = [int]$v.endpoint.Substring($i + 1)
    $peer = [ordered]@{ address = $wgHost; port = $port; public_key = $v.pub; allowed_ips = @('0.0.0.0/0', '::/0') }
    if ($v.psk) { $peer.pre_shared_key = $v.psk }
    if ($v.keepalive) { $peer.persistent_keepalive_interval = $v.keepalive }
    $mtu = if ($v.mtu) { $v.mtu } else { 1420 }
    $ep = [ordered]@{ type = 'wireguard'; tag = $tag; address = $v.address; private_key = $v.key; mtu = $mtu; peers = @($peer) }
    # e.g. a phone hotspot's 'Wi-Fi': school networks block WireGuard's UDP (phase 0 V2)
    if ($Bind) {
        if (-not $CheckOnly -and -not (Get-NetAdapter -Name $Bind -ErrorAction SilentlyContinue)) { Write-Host "[ERROR] Adapter '$Bind' not found (see Get-NetAdapter)."; exit 1 }
        $ep.bind_interface = $Bind
    }
    $dnsServer = if ($v.dns) { $v.dns[0] } else { '1.1.1.1' }
    $dns = [ordered]@{ type = 'udp'; tag = "$Name-dns"; server = $dnsServer; detour = $tag }
    return @{ Endpoint = $ep; Dns = $dns; ServerHost = $wgHost; Label = "WireGuard $($v.endpoint), DNS $dnsServer$(if ($Bind) { ", out of adapter '$Bind'" })" }
}

function Get-HostRule([string]$h) {
    if ($h -match '^\d+\.\d+\.\d+\.\d+$') { return [ordered]@{ ip_cidr = @("$h/32"); action = 'route'; outbound = 'direct' } }
    return [ordered]@{ domain = @($h); action = 'route'; outbound = 'direct' }
}

function Get-ExitRule([string]$Tag, $Domains, $Cidrs) {
    # A rule without conditions would match everything: refuse instead.
    if (-not $Domains -and -not $Cidrs) { Write-Host "[ERROR] $Tag has no test domains and no networks"; exit 1 }
    $r = [ordered]@{}
    if ($Domains) { $r.domain_suffix = @($Domains) }
    if ($Cidrs) { $r.ip_cidr = @($Cidrs) }
    $r.action = 'route'; $r.outbound = $Tag
    return $r
}

$a = New-Exit 'a' $AKind $AServer $AUser $AGroup $ABind $AConf
$b = New-Exit 'b' $BKind $BServer $BUser $BGroup $BBind $BConf
try {
    $dnsRules = @()
    if ($ADomains) { $dnsRules += [ordered]@{ domain_suffix = @($ADomains); server = 'a-dns' } }
    if ($BDomains) { $dnsRules += [ordered]@{ domain_suffix = @($BDomains); server = 'b-dns' } }
    $config = [ordered]@{
        log       = [ordered]@{ level = $LogLevel; timestamp = $true }
        dns       = [ordered]@{ servers = @([ordered]@{ type = 'local'; tag = 'local' }, $a.Dns, $b.Dns); rules = $dnsRules; final = 'local'; strategy = 'ipv4_only' }
        inbounds  = @([ordered]@{
            type = 'tun'; tag = 'tun-in'; interface_name = 'SplitGate-TUN'
            address = @('172.19.0.1/30', 'fdfe:dcba:9876::1/126'); auto_route = $true; strict_route = $true
        })
        endpoints = @($a.Endpoint, $b.Endpoint)
        outbounds = @([ordered]@{ type = 'direct'; tag = 'direct' })
        route     = [ordered]@{
            rules                   = @(
                [ordered]@{ action = 'sniff' },
                [ordered]@{ protocol = 'dns'; action = 'hijack-dns' },
                [ordered]@{ ip_version = 6; action = 'reject' },
                (Get-HostRule $a.ServerHost),
                (Get-HostRule $b.ServerHost),
                (Get-ExitRule 'vpn-a' $ADomains $ACidrs),
                (Get-ExitRule 'vpn-b' $BDomains $BCidrs)
            )
            final                   = 'direct'
            auto_detect_interface   = $true
            default_domain_resolver = [ordered]@{ server = 'local' }
        }
    }
    $json = $config | ConvertTo-Json -Depth 10
    $a.Endpoint.Clear(); $b.Endpoint.Clear(); $config = $null

    Write-Host "[config] exit A: $($a.Label)"
    Write-Host "[config]   via A: $((@($ADomains) + @($ACidrs)) -join ', ')"
    Write-Host "[config] exit B: $($b.Label)"
    Write-Host "[config]   via B: $((@($BDomains) + @($BCidrs)) -join ', ')"
    Write-Host '[config] everything else: direct'

    $ErrorActionPreference = 'Continue'
    $rc = Invoke-SingBoxStdin -Mode check -ConfigJson $json
    if ($rc -ne 0) { Write-Host '[ERROR] sing-box check failed, not starting.'; exit 1 }
    if ($CheckOnly) { Write-Host '[check-only] config OK'; exit 0 }

    & (Join-Path $Phase0Dir 'snapshot.ps1') -Label 'v9-before' | Out-Null
    New-Item -ItemType Directory -Force $LogDir | Out-Null
    $log = Join-Path $LogDir ("v9-two-exits-" + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
    Write-Host "[start] sing-box (log: $log). Wait until both exits log that they are up, then run test.ps1"
    Write-Host '        in a SECOND PowerShell. Press Ctrl+C here to stop.'
    $rc = Invoke-SingBoxStdin -Mode run -ConfigJson $json -LogFile $log
    Write-Host "[exit] sing-box exited with code $rc"
} finally {
    $json = $null
}
