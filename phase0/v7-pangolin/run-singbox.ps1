# V7 step 3: sing-box TUN; only ifconfig.me goes out through the official Pangolin client's adapter
# (direct outbound with bind_interface), everything else direct. No credentials involved.
# Run in an ADMINISTRATOR PowerShell, with the Pangolin app connected (Exit Node active) and its
# full-tunnel routes removed (drop-routes.ps1 -Apply). Stops with Ctrl+C.
# If anything goes wrong: phase0\v7-pangolin\cleanup.ps1 (does NOT touch the Pangolin app's adapter)
param(
    [string]$Interface = 'Pangolin',
    [string[]]$VpnDomains = @('ifconfig.me'),
    # DNS for rule domains, sent through the Pangolin adapter
    [string]$VpnDns = '1.1.1.1',
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
}

$config = [ordered]@{
    log       = [ordered]@{ level = $LogLevel; timestamp = $true }
    dns       = [ordered]@{
        servers  = @([ordered]@{ type = 'local'; tag = 'local' }, [ordered]@{ type = 'udp'; tag = 'vpn-dns'; server = $VpnDns; detour = 'vpn' })
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
        [ordered]@{ type = 'direct'; tag = 'vpn'; bind_interface = $Interface }
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
Write-Host "[config] via $Interface (bind_interface): $($VpnDomains -join ', '); vpn-dns $VpnDns through it"
$ErrorActionPreference = 'Continue'
$rc = Invoke-SingBoxStdin -Mode check -ConfigJson $json
if ($rc -ne 0) { Write-Host '[ERROR] sing-box check failed, not starting.'; exit 1 }
if ($CheckOnly) { Write-Host '[check-only] config OK'; exit 0 }

& (Join-Path $Phase0Dir 'snapshot.ps1') -Label 'v7-singbox-before' | Out-Null
New-Item -ItemType Directory -Force $LogDir | Out-Null
$log = Join-Path $LogDir ("v7-singbox-" + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
Write-Host "[start] sing-box (log: $log). Press Ctrl+C to stop."
$rc = Invoke-SingBoxStdin -Mode run -ConfigJson $json -LogFile $log
Write-Host "[exit] sing-box exited with code $rc"
