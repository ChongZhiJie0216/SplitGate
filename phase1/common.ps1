# Shared helpers for phase1 acceptance scripts (dot-source this file). Read-only functions only.
. (Join-Path $PSScriptRoot '..\phase0\common.ps1')

$Phase1Dir = $PSScriptRoot
$Phase1LogDir = Join-Path $Phase1Dir 'logs'
$TunName = 'SplitGate-TUN'

# Same rule as src-tauri/src/paths.rs: SPLITGATE_HOME wins, else %APPDATA%\SplitGate.
function Get-DataDir {
    if ($env:SPLITGATE_HOME) { return $env:SPLITGATE_HOME }
    return (Join-Path $env:APPDATA 'SplitGate')
}

# TUN network from settings.json (default 172.19.0.1/30), as "a.b.c." prefix for route matching.
function Get-TunPrefix {
    $tun = '172.19.0.1/30'
    $f = Join-Path (Get-DataDir) 'settings.json'
    if (Test-Path $f) {
        try { $s = Get-Content $f -Raw | ConvertFrom-Json; if ($s.tun_ipv4) { $tun = [string]$s.tun_ipv4 } } catch { }
    }
    $ip = ($tun -split '/')[0]
    $parts = $ip -split '\.'
    return ($parts[0..2] -join '.') + '.'
}

function Test-Connected {
    return [bool](Get-NetAdapter -IncludeHidden -Name $TunName -ErrorAction SilentlyContinue)
}

# Network state that must come back after a disconnect: default routes and static DNS.
function Get-NetState {
    [pscustomobject]@{
        DefaultRoutes = @(Get-DefaultRoutes | Where-Object { $_.InterfaceAlias -notlike $OurAdapterPattern } |
                ForEach-Object { "$($_.DestinationPrefix)|$($_.NextHop)|$($_.InterfaceAlias)" } | Sort-Object)
        StaticDns     = @(Get-StaticDnsState | Where-Object { $_.Alias -notlike $OurAdapterPattern } |
                ForEach-Object { "$($_.Alias)=$($_.NameServer)" } | Sort-Object)
    }
}

# Same shape as Get-NetState, from phase0\logs\baseline.json (taken while disconnected); $null if missing.
function Get-BaselineState {
    if (-not (Test-Path $BaselinePath)) { return $null }
    $b = Get-Content $BaselinePath -Raw | ConvertFrom-Json
    [pscustomobject]@{
        DefaultRoutes = @($b.DefaultRoutes | Where-Object { $_.InterfaceAlias -notlike $OurAdapterPattern } |
                ForEach-Object { "$($_.DestinationPrefix)|$($_.NextHop)|$($_.InterfaceAlias)" } | Sort-Object)
        StaticDns     = @($b.StaticDns | Where-Object { $_.Alias -notlike $OurAdapterPattern } |
                ForEach-Object { "$($_.Alias)=$($_.NameServer)" } | Sort-Object)
    }
}

# Everything SplitGate may leave behind. Empty list = clean.
# $Base (from Get-NetState while disconnected) adds the default route / DNS comparison.
function Get-Residue($Base) {
    $items = @()
    foreach ($p in @(Get-Process -Name 'sing-box' -ErrorAction SilentlyContinue)) { $items += "process sing-box pid $($p.Id) $($p.Path)" }
    foreach ($a in @(Get-OurAdapters)) { $items += "adapter $($a.Name) ($($a.Status))" }
    $prefix = Get-TunPrefix
    foreach ($r in @(Get-NetRoute -ErrorAction SilentlyContinue | Where-Object {
                $_.InterfaceAlias -like $OurAdapterPattern -or $_.DestinationPrefix.StartsWith($prefix) -or $_.NextHop.StartsWith($prefix) })) {
        $items += "route $($r.DestinationPrefix) via $($r.NextHop) on $($r.InterfaceAlias)"
    }
    if ($null -ne $Base) {
        $now = Get-NetState
        # Plain loops: Compare-Object in PowerShell 5.1 refuses an empty list.
        foreach ($k in @($now.DefaultRoutes)) { if (@($Base.DefaultRoutes) -notcontains $k) { $items += "default route added: $k" } }
        foreach ($k in @($Base.DefaultRoutes)) { if (@($now.DefaultRoutes) -notcontains $k) { $items += "default route missing: $k" } }
        foreach ($k in @($now.StaticDns)) { if (@($Base.StaticDns) -notcontains $k) { $items += "DNS changed, now: $k" } }
    }
    return $items
}

# Direct internet works: DNS lookup + TCP connect, both within $TimeoutMs.
function Test-DirectNet([string]$HostName = 'www.msftconnecttest.com', [int]$Port = 80, [int]$TimeoutMs = 1500) {
    $c = New-Object System.Net.Sockets.TcpClient
    try {
        $t = $c.ConnectAsync($HostName, $Port)
        return ($t.Wait($TimeoutMs) -and $c.Connected)
    } catch { return $false } finally { $c.Close() }
}

function Write-Log([string]$File, [string]$Text) {
    New-Item -ItemType Directory -Force $Phase1LogDir | Out-Null
    $line = "$(Get-Date -Format 'HH:mm:ss.fff') $Text"
    Write-Host $line
    Add-Content -Path (Join-Path $Phase1LogDir $File) -Value $line -Encoding utf8
}
