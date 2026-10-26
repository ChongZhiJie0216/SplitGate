# Copy the pinned sing-box into src-tauri\binaries with the name Tauri expects for a sidecar
# (sing-box-<target-triple>.exe) and copy its license text into third_party\licenses.
# Run third_party\fetch.ps1 first. Binaries are not in git (see CLAUDE.md).
# libcronet.dll from the sing-box zip is NOT copied: it is only for the naive outbound (tested 2026-10-26).
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$expected = '1.14.4'
$src = Join-Path $root 'third_party\bin\sing-box\sing-box.exe'
if (-not (Test-Path $src)) { Write-Host "[ERROR] $src not found. Run: powershell -ExecutionPolicy Bypass -File third_party\fetch.ps1"; exit 1 }

$ver = (& $src version | Select-Object -First 1) -replace '^sing-box version ', ''
if ($ver -ne $expected) { Write-Host "[ERROR] sing-box is $ver, expected $expected (third_party\VERSIONS.md)."; exit 1 }

$triple = 'x86_64-pc-windows-msvc'
$binDir = Join-Path $root 'src-tauri\binaries'
New-Item -ItemType Directory -Force $binDir | Out-Null
Copy-Item $src (Join-Path $binDir "sing-box-$triple.exe") -Force

$lic = Join-Path $root 'third_party\licenses'
New-Item -ItemType Directory -Force $lic | Out-Null
Copy-Item (Join-Path $root 'third_party\bin\sing-box\LICENSE') (Join-Path $lic 'sing-box-LICENSE.txt') -Force

# tauri dev runs the exe from target\debug; the sidecar must sit next to it there too.
$dbg = Join-Path $root 'src-tauri\target\debug'
New-Item -ItemType Directory -Force $dbg | Out-Null
Copy-Item $src (Join-Path $dbg 'sing-box.exe') -Force

Write-Host "[ok] sing-box $ver -> src-tauri\binaries\sing-box-$triple.exe (and target\debug\sing-box.exe)"
