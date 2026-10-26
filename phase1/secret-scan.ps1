# Phase 1 acceptance: "the password cannot be found in logs or the settings folder". READ-ONLY.
# You type each secret (VPN password, WireGuard private key, ...) at a hidden prompt; empty line ends.
# Secrets stay in memory only: never printed, never written to the log or any file.
# Searched, as UTF-8 and as UTF-16: every file under the data folder (data\ next to SplitGate.exe or
# SPLITGATE_HOME, zips such as diagnostics packages are unpacked to %TEMP% and searched too),
# the old settings folder %APPDATA%\SplitGate (up to v0.11),
# %LOCALAPPDATA%\SplitGate (WebView2 data, old tunnel.json), any -Extra folder,
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

$roots = @((Get-DataDir), $OldDataDir, (Join-Path $env:LOCALAPPDATA 'SplitGate')) + $Extra
$hits = 0; $files = 0; $skipped = @()

function Search-File($f, [string]$label) {
    if ($f.Length -gt $MaxMB * 1MB) { $script:skipped += "$label (over $MaxMB MB)"; return }
    try {
        $fs = [IO.File]::Open($f.FullName, 'Open', 'Read', 'ReadWrite, Delete')
        try { $buf = New-Object byte[] $fs.Length; [void]$fs.Read($buf, 0, $buf.Length) } finally { $fs.Close() }
    } catch { $script:skipped += "$label (cannot read: in use?)"; return }
    $script:files++
    $hay = $latin1.GetString($buf)
    foreach ($n in $needles) {
        if ($hay.IndexOf($n.Text, [StringComparison]::Ordinal) -ge 0) { $script:hits++; Write-Host "[FOUND] secret #$($n.Id) ($($n.Enc)) in $label" }
    }
}

foreach ($root in $roots) {
    if (-not (Test-Path $root)) { Write-Host "[skip] $root (does not exist)"; continue }
    Write-Host "[scan] $root"
    foreach ($f in @(Get-ChildItem $root -File -Recurse -Force -ErrorAction SilentlyContinue)) {
        Search-File $f $f.FullName
        # Diagnostics packages are zips: compressed text cannot be searched, so unpack them first.
        if ($f.Extension -ieq '.zip') {
            $tmp = Join-Path $env:TEMP "sg-secret-scan-$([guid]::NewGuid().ToString('N'))"
            try {
                Expand-Archive -Path $f.FullName -DestinationPath $tmp -Force
                foreach ($z in @(Get-ChildItem $tmp -File -Recurse -Force)) { Search-File $z "$($f.FullName) > $($z.FullName.Substring($tmp.Length + 1))" }
            } catch { $skipped += "$($f.FullName) (cannot unpack)" }
            finally { Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue }
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
