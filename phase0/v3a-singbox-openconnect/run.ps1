# V3a: Cisco/AnyConnect-compatible exit through sing-box's BUILT-IN OpenConnect endpoint (sing-box 1.14+).
# No openconnect.exe, no wintun adapter for the VPN, no vpnc-script. Only ifconfig.me goes through the VPN.
# Run in an ADMINISTRATOR PowerShell. You type the password; it is kept in memory and handed to sing-box
# on STDIN - it never goes to disk, the command line or the log. Stops with Ctrl+C.
# If anything goes wrong: phase0\v3a-singbox-openconnect\cleanup.ps1
param(
    [string]$Server = 'https://sslvpn.jiecloud.net:8443',
    [string]$User = '',
    # Preselect the login GROUP (AnyLink shows: Master Network / Test Network / All Tunnel / ...). Empty = ask.
    [string]$AuthGroup = '',
    [string[]]$VpnDomains = @('ifconfig.me'),
    # Empty = use the DNS servers the VPN pushes (sing-box "openconnect" DNS server). Or give an IP, e.g. 1.1.1.1
    [string]$VpnDns = '',
    # Disable the DTLS (UDP) data channel and stay on TLS (TCP) only
    [switch]$NoUdp,
    [ValidateSet('trace', 'debug', 'info', 'warn', 'error')][string]$LogLevel = 'info',
    # Developer check: build the config with a dummy user/password, run "sing-box check" and stop. No admin needed.
    [switch]$CheckOnly
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\common.ps1')
. (Join-Path $Phase0Dir 'stdin-run.ps1')
if ($CheckOnly) {
    $User = 'test-user'
    $secure = ConvertTo-SecureString 'dummy-password' -AsPlainText -Force
} else {
    if (-not (Test-IsAdmin)) { Write-Host '[ERROR] Not Administrator. Right-click PowerShell > Run as administrator, then run this again.'; exit 1 }
    Write-Host '[ok] running as Administrator'
    if (-not $User) { $User = Read-Host 'VPN username' }
    $secure = Read-Host "VPN password for $User (not shown)" -AsSecureString
}
$bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)

try {
    $oc = [ordered]@{
        type     = 'openconnect'
        tag      = 'vpn'
        server   = $Server
        username = $User
        password = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr)
    }
    if ($AuthGroup) { $oc.auth_group = $AuthGroup }
    if ($NoUdp) { $oc.no_udp = $true }

    if ($VpnDns) { $vpnDnsServer = [ordered]@{ type = 'udp'; tag = 'vpn-dns'; server = $VpnDns; detour = 'vpn' } }
    else { $vpnDnsServer = [ordered]@{ type = 'openconnect'; tag = 'vpn-dns'; endpoint = 'vpn'; accept_default_resolvers = $true } }

    $config = [ordered]@{
        log       = [ordered]@{ level = $LogLevel; timestamp = $true }
        dns       = [ordered]@{
            servers  = @([ordered]@{ type = 'local'; tag = 'local' }, $vpnDnsServer)
            rules    = @([ordered]@{ domain_suffix = $VpnDomains; server = 'vpn-dns' })
            final    = 'local'
            strategy = 'ipv4_only'
        }
        inbounds  = @([ordered]@{
            type = 'tun'; tag = 'tun-in'; interface_name = 'SplitGate-TUN'
            address = @('172.19.0.1/30', 'fdfe:dcba:9876::1/126'); auto_route = $true; strict_route = $true
        })
        endpoints = @($oc)
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
    # sing-box API (gRPC) on loopback with a random port and secret, so a second window can run
    # "sing-box api openconnect status|auth" to see / answer the login form the server sends.
    $apiPort = Get-Random -Minimum 20000 -Maximum 60000
    $apiSecretBytes = New-Object byte[] 16
    [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($apiSecretBytes)
    $apiSecret = -join ($apiSecretBytes | ForEach-Object { $_.ToString('x2') })
    $config.services = @([ordered]@{ type = 'api'; tag = 'api'; listen = '127.0.0.1'; listen_port = $apiPort; secret = $apiSecret })

    $json = $config | ConvertTo-Json -Depth 10
    $oc.password = $null; $config = $null

    if (-not $CheckOnly) { & (Join-Path $Phase0Dir 'snapshot.ps1') -Label 'v3a-before' | Out-Null }
    Write-Host "[config] server $Server, user $User, group $(if ($AuthGroup) { $AuthGroup } else { '(ask)' }), via VPN: $($VpnDomains -join ', ')"
    Write-Host ("[config] vpn-dns: " + $(if ($VpnDns) { "$VpnDns through the tunnel" } else { 'servers pushed by the VPN' }) + $(if ($NoUdp) { '; DTLS off' } else { '' }))

    $ErrorActionPreference = 'Continue'
    $rc = Invoke-SingBoxStdin -Mode check -ConfigJson $json
    if ($rc -ne 0) { Write-Host '[ERROR] sing-box check failed, not starting.'; exit 1 }
    if ($CheckOnly) { Write-Host '[check-only] config OK'; exit 0 }

    $log = Join-Path $LogDir ("v3a-singbox-" + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
    New-Item -ItemType Directory -Force $LogDir | Out-Null
    Write-Host '[api] To see / answer the login form, paste these 3 lines into a SECOND PowerShell (same folder):'
    Write-Host "      `$env:BOX_API_URL = '127.0.0.1:$apiPort'; `$env:BOX_API_SECRET = '$apiSecret'"
    Write-Host '      .\third_party\bin\sing-box\sing-box.exe api openconnect status'
    Write-Host '      .\third_party\bin\sing-box\sing-box.exe api openconnect auth'
    # Let a helper (or Claude Code) in another window read the login form. Session-only secret, deleted on exit.
    New-Item -ItemType Directory -Force $RuntimeDir | Out-Null
    $apiEnv = Join-Path $RuntimeDir 'v3a-api.env'
    [IO.File]::WriteAllText($apiEnv, "BOX_API_URL=127.0.0.1:$apiPort`r`nBOX_API_SECRET=$apiSecret`r`n", (New-Object Text.UTF8Encoding $false))
    Write-Host "[start] sing-box (log: $log). Press Ctrl+C to stop."
    $rc = Invoke-SingBoxStdin -Mode run -ConfigJson $json -LogFile $log
    Write-Host "[exit] sing-box exited with code $rc"
} finally {
    if ($apiEnv) { Remove-Item -Force $apiEnv -ErrorAction SilentlyContinue }
    $json = $null
    [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr)
}
