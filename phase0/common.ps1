# Shared helpers for phase0 scripts (dot-source this file). Read-only functions only.
$ErrorActionPreference = 'Stop'

$Phase0Dir = $PSScriptRoot
$RepoRoot = Split-Path $Phase0Dir -Parent
$BinDir = Join-Path $RepoRoot 'third_party\bin'
$LogDir = Join-Path $Phase0Dir 'logs'
$RuntimeDir = Join-Path $Phase0Dir 'runtime'
$BaselinePath = Join-Path $LogDir 'baseline.json'

# Every adapter SplitGate creates must match this pattern, so cleanup can find it.
$OurAdapterPattern = 'SplitGate*'
$OurProcessNames = @('sing-box', 'openconnect', 'olm', 'pangolin-cli')

function Test-IsAdmin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    return ([Security.Principal.WindowsPrincipal]::new($id)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# Static DNS servers per interface as stored in the registry (empty = DHCP / automatic).
function Get-StaticDnsState {
    $base = 'HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces'
    $result = @()
    foreach ($a in Get-NetAdapter -IncludeHidden -ErrorAction SilentlyContinue) {
        $key = Join-Path $base $a.InterfaceGuid
        $ns = ''
        if (Test-Path $key) { $ns = [string](Get-ItemProperty $key -Name NameServer -ErrorAction SilentlyContinue).NameServer }
        $result += [pscustomobject]@{ Alias = $a.Name; Guid = $a.InterfaceGuid; Description = $a.InterfaceDescription; NameServer = $ns }
    }
    return $result
}

function Get-DefaultRoutes {
    Get-NetRoute -ErrorAction SilentlyContinue | Where-Object { $_.DestinationPrefix -in @('0.0.0.0/0', '::/0', '0.0.0.0/1', '128.0.0.0/1') } |
        Select-Object DestinationPrefix, NextHop, InterfaceAlias, InterfaceIndex, RouteMetric
}

function Get-OurProcesses {
    Get-Process -Name $OurProcessNames -ErrorAction SilentlyContinue | Select-Object Id, ProcessName, Path
}

function Get-OurAdapters {
    Get-NetAdapter -IncludeHidden -Name $OurAdapterPattern -ErrorAction SilentlyContinue
}
