# Publish the current version as a GitHub Release, so "check for updates" in SplitGate can see it.
# Uploads portable\SplitGate-<version>-portable.zip and its .sha256 (build both first with build.bat;
# the in-app updater needs the checksum, v0.19.0) and uses that
# version's section of CHANGELOG.md as the release notes. Needs the GitHub CLI (gh), logged in.
# The repo is public: the release (zip + notes) is visible to everyone.
# Default: CHECK only (prints what it would do). Add -Yes to really create the release.
# Usage: powershell -ExecutionPolicy Bypass -File scripts\publish-release.ps1 [-Yes]
param([switch]$Yes)
# 'Continue': every step checks its own result; with 'Stop', PowerShell 5.1 turns a native tool's
# stderr line into an error and the friendly messages below would never show.
$ErrorActionPreference = 'Continue'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
function Fail($msg) { Write-Host "[ERROR] $msg"; exit 1 }

$version = (Get-Content (Join-Path $root 'package.json') -Raw | ConvertFrom-Json).version
$tag = "v$version"
$zip = Join-Path $root "portable\SplitGate-$version-portable.zip"
Write-Host "=== SplitGate $tag ==="

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { Fail 'GitHub CLI (gh) not found. Install it from https://cli.github.com/ and run: gh auth login' }
& gh auth status *> $null
if ($LASTEXITCODE -ne 0) { Fail 'gh is not logged in. Run: gh auth login' }
if (-not (Test-Path $zip)) { Fail "$zip not found. Run build.bat first." }
$sha = "$zip.sha256"
if (-not (Test-Path $sha)) { Fail "$sha not found. Run build.bat again (v0.19.0+ writes it)." }
$want = ((Get-Content $sha -Raw).Trim() -split '\s+')[0]
if ((Get-FileHash -Algorithm SHA256 -LiteralPath $zip).Hash -ne $want) { Fail "$sha does not match the zip. Run build.bat again." }

# The release must point at what is on main, so the tag, the zip and CHANGELOG.md agree.
$branch = (git rev-parse --abbrev-ref HEAD).Trim()
if ($branch -ne 'main') { Fail "current branch is '$branch'. Merge into main, check out main, build, then publish." }
if (git status --porcelain --untracked-files=no) { Fail 'uncommitted changes. Commit or stash them first.' }
git fetch -q origin main
if ((git rev-parse HEAD).Trim() -ne (git rev-parse origin/main).Trim()) { Fail 'local main differs from origin/main. Pull / push first.' }

& gh release view $tag *> $null
if ($LASTEXITCODE -eq 0) { Fail "release $tag already exists. Raise the version in package.json for a new one." }

# This version's CHANGELOG section ("## <version> ..." up to the next "## ").
$lines = Get-Content (Join-Path $root 'CHANGELOG.md') -Encoding UTF8
$notes = @(); $in = $false
foreach ($l in $lines) {
    if ($l -match '^## ') { $in = $l.StartsWith("## $version ") ; continue }
    if ($in) { $notes += $l }
}
$text = ($notes -join "`n").Trim()
if (-not $text) { Fail "no '## $version' section in CHANGELOG.md." }
$notesFile = Join-Path $env:TEMP "splitgate-release-notes-$version.md"
[IO.File]::WriteAllText($notesFile, $text, (New-Object Text.UTF8Encoding $false))

$mb = [Math]::Round((Get-Item $zip).Length / 1MB, 1)
Write-Host "release: $tag on main ($((git rev-parse --short HEAD).Trim()))"
Write-Host "asset:   $zip ($mb MB) + .sha256"
Write-Host "notes:   $notesFile ($($notes.Count) lines from CHANGELOG.md)"
if (-not $Yes) { Write-Host '[check] nothing published. Re-run with -Yes to create the release.'; exit 0 }

& gh release create $tag $zip $sha --target main --title "SplitGate $tag" --notes-file $notesFile
if ($LASTEXITCODE -ne 0) { Fail 'gh release create failed.' }
Remove-Item -Force $notesFile
Write-Host "[ok] published $tag. SplitGate on other PCs will show it under About > Check for updates."
