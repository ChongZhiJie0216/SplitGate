# V4 test: (1) the VPN's outer TCP connection uses the bound adapter, (2) split routing still works,
# (3) with -Kill: network recovery + leftover check. Read-only except -Kill (stops sing-box).
param(
    [string]$BindInterface = 'Wi-Fi',
    [string]$ServerIp = '202.186.213.216',
    [int]$ServerPort = 8443,
    [string]$HomeIp = '202.186.213.216',
    [switch]$Kill
)
. (Join-Path $PSScriptRoot '..\common.ps1')
if (-not (Get-OurProcesses)) { Write-Host '[ERROR] sing-box is not running. Start phase0\v4-bind-interface\run.ps1 first. Test NOT run.'; exit 1 }

$bound = @(Get-NetIPAddress -InterfaceAlias $BindInterface -AddressFamily IPv4 -ErrorAction SilentlyContinue | Select-Object -Expand IPAddress)
Write-Host "--- outer VPN connection (expect local address on '$BindInterface': $($bound -join ', ')) ---"
$conns = @(Get-NetTCPConnection -RemoteAddress $ServerIp -RemotePort $ServerPort -State Established -ErrorAction SilentlyContinue |
    Where-Object { $_.OwningProcess -in @(Get-OurProcesses | Select-Object -Expand Id) })
if (-not $conns) {
    Write-Host "[FAIL] no established sing-box TCP connection to ${ServerIp}:$ServerPort"
} else {
    foreach ($c in $conns) {
        $ok = $bound -contains $c.LocalAddress
        Write-Host ("[{0}] {1}:{2} -> {3}:{4}" -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $c.LocalAddress, $c.LocalPort, $c.RemoteAddress, $c.RemotePort)
    }
}
Write-Host '--- DTLS / UDP (from the newest sing-box log) ---'
$log = Get-ChildItem (Join-Path $LogDir 'v3a-singbox-*.log') -ErrorAction SilentlyContinue | Sort-Object LastWriteTime | Select-Object -Last 1
if ($log) { Select-String -Path $log.FullName -Pattern 'tunnel established|DTLS' | Select-Object -Last 3 | ForEach-Object { $_.Line } }

$splat = @{ HomeIp = $HomeIp }
if ($Kill) { $splat.Kill = $true }
& (Join-Path $Phase0Dir 'split-test.ps1') @splat
