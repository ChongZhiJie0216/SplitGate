# Phase 1 acceptance helper: find the real process of an app (e.g. Xbox App) for a rule preset. READ-ONLY.
# Open the app first and let it load (sign in, open the store), then run this.
# Lists processes whose name, path, description or package matches -Filter, with their open
# internet connections, and suggests process_name entries (Store apps live under a versioned
# WindowsApps folder, so process_path would break on the next update).
# Usage: powershell -ExecutionPolicy Bypass -File phase1\find-process.ps1 [-Filter xbox,gaming]
param([string[]]$Filter = @('xbox', 'gaming'))
. (Join-Path $PSScriptRoot 'common.ps1')

$conn = @{}
foreach ($c in @(Get-NetTCPConnection -State Established -ErrorAction SilentlyContinue | Where-Object { $_.RemoteAddress -notmatch '^(127\.|::1$|0\.)' })) {
    $conn[[int]$c.OwningProcess] = 1 + [int]$conn[[int]$c.OwningProcess]
}

$found = @()
foreach ($p in @(Get-Process -ErrorAction SilentlyContinue)) {
    $path = [string]$p.Path
    $desc = [string]$p.Description
    $text = "$($p.ProcessName) $path $desc"
    $match = $false
    foreach ($f in $Filter) { if ($text -like "*$f*") { $match = $true } }
    if (-not $match) { continue }
    $found += [pscustomobject]@{
        Name        = "$($p.ProcessName).exe"
        Pid         = $p.Id
        Connections = [int]$conn[[int]$p.Id]
        Store       = $path -like '*\WindowsApps\*'
        Description = $desc
        Path        = $path
    }
}
if ($found.Count -eq 0) { Write-Host "[none] no process matches: $($Filter -join ', '). Is the app open? (Run as admin to see paths of elevated processes.)"; exit 1 }
$found | Sort-Object Connections -Descending | Format-Table Name, Pid, Connections, Store, Description -AutoSize | Out-String -Width 200 | Write-Host
$found | Format-List Name, Path | Out-String -Width 400 | Write-Host
$names = @($found | Where-Object { $_.Connections -gt 0 } | Select-Object -ExpandProperty Name -Unique)
if ($names.Count -gt 0) { Write-Host "[suggest] process_name (have internet connections now): $($names -join ', ')" }
else { Write-Host '[suggest] none of them has an internet connection right now; use the app (open the store) and run again.' }
