# Build the portable SplitGate: portable\SplitGate\ (SplitGate.exe + sing-box.exe + licenses\)
# and portable\SplitGate-<version>-portable.zip. No installer. Settings still go to %APPDATA%\SplitGate.
# For quick testing, SplitGate.exe + sing-box.exe are also copied to the repo root (ignored by git).
# Usage: powershell -ExecutionPolicy Bypass -File scripts\build-portable.ps1
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

function Fail($msg) { Write-Host "[ERROR] $msg"; exit 1 }

Write-Host '[1/5] Check tools'
foreach ($t in 'node', 'npm', 'cargo') { if (-not (Get-Command $t -ErrorAction SilentlyContinue)) { Fail "$t not found. Install Node.js and Rust (rustup) first." } }
$version = (Get-Content (Join-Path $root 'package.json') -Raw | ConvertFrom-Json).version
# A running copy locks its exe files: the copy steps below would fail after a long build.
$targets = @((Join-Path $root 'portable\SplitGate'), $root)
foreach ($p in @(Get-Process -Name 'SplitGate', 'sing-box' -ErrorAction SilentlyContinue)) {
    if (-not $p.Path) { continue }
    $dir = Split-Path $p.Path -Parent
    if ($targets -contains $dir) { Fail "$($p.Path) is running. Quit SplitGate first (tray icon > Quit), then build again." }
}

# npm writes node_modules\.package-lock.json on every install: older than package-lock.json (or missing)
# means a dependency was added or changed since the last install, e.g. after git pull.
$installed = Join-Path $root 'node_modules\.package-lock.json'
$lock = Join-Path $root 'package-lock.json'
if (-not (Test-Path $installed) -or (Get-Item $installed).LastWriteTime -lt (Get-Item $lock).LastWriteTime) {
    Write-Host '      frontend dependencies changed: npm ci'
    npm ci --no-audit --no-fund
    if ($LASTEXITCODE -ne 0) { Fail 'npm ci failed.' }
}

Write-Host '[2/5] Prepare sing-box sidecar'
& powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'prepare-sidecar.ps1')
if ($LASTEXITCODE -ne 0) { Fail 'prepare-sidecar.ps1 failed.' }

Write-Host '[3/5] Run tests (build stops if any fail)'
Push-Location (Join-Path $root 'src-tauri')
cargo test --quiet
$rc = $LASTEXITCODE
Pop-Location
if ($rc -ne 0) { Fail 'cargo test failed.' }

Write-Host '[4/5] Build release exe (no installer)'
npx tauri build --no-bundle
if ($LASTEXITCODE -ne 0) { Fail 'tauri build failed.' }

Write-Host '[5/5] Assemble portable folder'
$rel = Join-Path $root 'src-tauri\target\release'
$out = Join-Path $root 'portable\SplitGate'
if (Test-Path $out) { Remove-Item -Recurse -Force $out }
New-Item -ItemType Directory -Force (Join-Path $out 'licenses') | Out-Null
Copy-Item (Join-Path $rel 'splitgate.exe') (Join-Path $out 'SplitGate.exe')
Copy-Item (Join-Path $root 'third_party\bin\sing-box\sing-box.exe') (Join-Path $out 'sing-box.exe')
Copy-Item (Join-Path $root 'third_party\licenses\*') (Join-Path $out 'licenses')
$zip = Join-Path $root "portable\SplitGate-$version-portable.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $out -DestinationPath $zip
Copy-Item (Join-Path $out 'SplitGate.exe') (Join-Path $root 'SplitGate.exe') -Force
Copy-Item (Join-Path $out 'sing-box.exe') (Join-Path $root 'sing-box.exe') -Force

$mb = [Math]::Round((Get-Item (Join-Path $out 'SplitGate.exe')).Length / 1MB, 1)
Write-Host "[ok] SplitGate v$version"
Write-Host "     folder: $out (SplitGate.exe $mb MB + sing-box.exe)"
Write-Host "     zip:    $zip"
Write-Host "     test:   $(Join-Path $root 'SplitGate.exe') (copy in the repo root, run it as Administrator)"
