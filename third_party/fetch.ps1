# Download pinned third-party binaries, verify SHA256, extract to third_party\bin.
# Versions and sources: third_party\VERSIONS.md (keep both in sync).
# Usage: powershell -ExecutionPolicy Bypass -File third_party\fetch.ps1
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$dl = Join-Path $root 'downloads'
$bin = Join-Path $root 'bin'
$sevenZip = 'C:\Program Files\7-Zip\7z.exe'

$items = @(
    @{ Name = 'sing-box'; File = 'sing-box-1.14.4-windows-amd64.zip'
       Url = 'https://github.com/SagerNet/sing-box/releases/download/v1.14.4/sing-box-1.14.4-windows-amd64.zip'
       Sha = '99185d75d314a07dbba4a130c8ad986cbeb629a07927975636b341ca61f991df' },
    @{ Name = 'openconnect'; File = 'openconnect-installer-MinGW64-GnuTLS-v9.21.exe'
       Url = 'https://gitlab.com/openconnect/openconnect/-/jobs/14877945030/artifacts/raw/openconnect-installer-MinGW64-GnuTLS.exe'
       Sha = '6ee9e8eb9bc59ef70bb0717df7f99703f8a2ccd11d8e45d58a61f9a2e6ef7d00' },
    @{ Name = 'wintun'; File = 'wintun-0.14.1.zip'
       Url = 'https://www.wintun.net/builds/wintun-0.14.1.zip'
       Sha = '07c256185d6ee3652e09fa55c0b673e2624b565e02c4b9091c79ca7d2f24ef51' },
    @{ Name = 'olm'; File = 'olm_windows_amd64.exe'
       Url = 'https://github.com/fosrl/olm/releases/download/1.10.1/olm_windows_amd64.exe'
       Sha = '7ca1ca7f3e64d4590e9c93e57f99039727117f9ac403025f2f386af897e23563' }
)

if (-not (Test-Path $sevenZip)) { throw "7-Zip not found at $sevenZip (needed to unpack the openconnect NSIS installer)." }
New-Item -ItemType Directory -Force $dl, $bin | Out-Null

foreach ($i in $items) {
    $path = Join-Path $dl $i.File
    if (-not (Test-Path $path)) {
        Write-Host "[download] $($i.File)"
        curl.exe -sSL --fail -o $path $i.Url
        if ($LASTEXITCODE -ne 0) { throw "Download failed: $($i.Url)" }
    }
    $hash = (Get-FileHash $path -Algorithm SHA256).Hash.ToLower()
    if ($hash -ne $i.Sha) { throw "SHA256 mismatch for $($i.File): got $hash, expected $($i.Sha)" }
    Write-Host "[ok] $($i.File) $hash"

    $out = Join-Path $bin $i.Name
    if (Test-Path $out) { Remove-Item -Recurse -Force $out }
    New-Item -ItemType Directory -Force $out | Out-Null
    switch ($i.Name) {
        'sing-box' {
            & $sevenZip x -y -bso0 "-o$out" $path
            $inner = Get-ChildItem $out -Directory | Select-Object -First 1
            Move-Item (Join-Path $inner.FullName '*') $out -Force
            Remove-Item $inner.FullName
        }
        'openconnect' {
            & $sevenZip x -y -bso0 "-o$out" $path
            Remove-Item -Recurse -Force (Join-Path $out '$PLUGINSDIR'), (Join-Path $out 'Uninstall OpenConnect.exe') -ErrorAction SilentlyContinue
        }
        'wintun' { & $sevenZip x -y -bso0 "-o$out" $path }
        'olm' { Copy-Item $path (Join-Path $out 'olm.exe') }
    }
}
Write-Host "[done] binaries in $bin"
