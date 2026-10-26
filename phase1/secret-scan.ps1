# Phase 1 acceptance: "the password cannot be found in logs or %APPDATA%\SplitGate". READ-ONLY.
# You type each secret (VPN password, WireGuard private key, ...) at a hidden prompt; empty line ends.
# Secrets stay in memory only: never printed, never written to the log or any file.
# Searched, as UTF-8 and as UTF-16: every file under the data folder (%APPDATA%\SplitGate or
# SPLITGATE_HOME), %LOCALAPPDATA%\SplitGate (WebView2 data, old tunnel.json), any -Extra folder,
# plus the command lines of running SplitGate / sing-box and the SplitGate scheduled task.
# Usage: powershell -ExecutionPolicy Bypass -File phase1\secret-scan.ps1 [-Extra C:\path\portable]
param([string[]]$Extra = @(), [int]$MaxMB = 100)
. (Join-Path $PSScriptRoot 'common.ps1')

$secrets = @()
while ($true) {
    $ss = Read-Host -AsSecureString "Secret #$($secrets.Count + 1) (empty = start scan)"
    $bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($ss)
    try { $plain = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr) } finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr) }
    if ([string]::IsNullOrEmpty($plain)) { break }
    if ($plain.Length -lt 4) { Write-Host '[warn] shorter than 4 characters: too many false hits, skipped.'; continue }
    $secrets += $plain
}
if ($secrets.Count -eq 0) { Write-Host '[ERROR] no secret entered.'; exit 1 }

# Latin-1 maps every byte to one char, so a byte search becomes an ordinal string search.
$latin1 = [Text.Encoding]::GetEncoding(28591)
$needles = @()
for ($i = 0; $i -lt $secrets.Count; $i++) {
    $needles += [pscustomobject]@{ Id = $i + 1; Enc = 'UTF-8'; Text = $latin1.GetString([Text.Encoding]::UTF8.GetBytes($secrets[$i])) }
    $needles += [pscustomobject]@{ Id = $i + 1; Enc = 'UTF-16'; Text = $latin1.GetString([Text.Encoding]::Unicode.GetBytes($secrets[$i])) }
}

$roots = @((Get-DataDir), (Join-Path $env:LOCALAPPDATA 'SplitGate')) + $Extra
$hits = 0; $files = 0; $skipped = @()
foreach ($root in $roots) {
    if (-not (Test-Path $root)) { Write-Host "[skip] $root (does not exist)"; continue }
    Write-Host "[scan] $root"
    foreach ($f in @(Get-ChildItem $root -File -Recurse -Force -ErrorAction SilentlyContinue)) {
        if ($f.Length -gt $MaxMB * 1MB) { $skipped += "$($f.FullName) (over $MaxMB MB)"; continue }
        try {
            $fs = [IO.File]::Open($f.FullName, 'Open', 'Read', 'ReadWrite, Delete')
            try { $buf = New-Object byte[] $fs.Length; [void]$fs.Read($buf, 0, $buf.Length) } finally { $fs.Close() }
        } catch { $skipped += "$($f.FullName) (cannot read: in use?)"; continue }
        $files++
        $hay = $latin1.GetString($buf)
        foreach ($n in $needles) {
            if ($hay.IndexOf($n.Text, [StringComparison]::Ordinal) -ge 0) { $hits++; Write-Host "[FOUND] secret #$($n.Id) ($($n.Enc)) in $($f.FullName)" }
        }
    }
}

# Command lines: a secret must never be passed as an argument.
foreach ($p in @(Get-CimInstance Win32_Process -Filter "Name='SplitGate.exe' OR Name='sing-box.exe'" -ErrorAction SilentlyContinue)) {
    for ($i = 0; $i -lt $secrets.Count; $i++) {
        if ([string]$p.CommandLine -and ([string]$p.CommandLine).Contains($secrets[$i])) { $hits++; Write-Host "[FOUND] secret #$($i + 1) in command line of $($p.Name) pid $($p.ProcessId)" }
    }
}
# 'Continue': with 'Stop', PowerShell 5.1 turns the 'task not found' stderr line into an error.
$ErrorActionPreference = 'Continue'
$xml = schtasks /query /tn SplitGate /xml 2>$null | Out-String
$ErrorActionPreference = 'Stop'
if ($xml) {
    for ($i = 0; $i -lt $secrets.Count; $i++) { if ($xml.Contains($secrets[$i])) { $hits++; Write-Host "[FOUND] secret #$($i + 1) in scheduled task SplitGate" } }
}
$secrets = $null; $needles = $null

foreach ($s in $skipped) { Write-Host "[skipped] $s" }
if ($hits -eq 0) { Write-Host "[PASS] $files file(s) scanned, no secret found." ; exit 0 }
Write-Host "[FAIL] $hits hit(s). Tell the maintainer which file; do not paste its content anywhere."
exit 1
