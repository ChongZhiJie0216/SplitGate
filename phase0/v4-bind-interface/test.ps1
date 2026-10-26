# V4 test: (1) the VPN's outer TCP connection uses the bound adapter, (2) split routing still works,
# (3) with -Kill: network recovery + leftover check. Read-only except -Kill (stops sing-box).
param(
    [string]$BindInterface = 'Wi-Fi',
    # Cisco server IP and home public IP: pass them in, they are not stored in the public repo.
    [Parameter(Mandatory = $true)][string]$ServerIp,
    [int]$ServerPort = 8443,
    [Parameter(Mandatory = $true)][string]$HomeIp,
    [switch]$Kill
)
. (Join-Path $PSScriptRoot '..\common.ps1')
if (-not (Get-OurProcesses)) { Write-Host '[ERROR] sing-box is not running. Start phase0\v4-bind-interface\run.ps1 first. Test NOT run.'; exit 1 }
$log = Get-ChildItem (Join-Path $LogDir 'v3a-singbox-*.log') -ErrorAction SilentlyContinue | Sort-Object LastWriteTime | Select-Object -Last 1
if (-not $log -or -not (Select-String -Path $log.FullName -Pattern 'tunnel established' -Quiet)) {
    Write-Host '[ERROR] VPN not connected yet (no "tunnel established" in the sing-box log). Wait for it, then run again. Test NOT run.'; exit 1
}

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
if ($log) { Select-String -Path $log.FullName -Pattern 'tunnel established|DTLS' | Select-Object -Last 3 | ForEach-Object { $_.Line } }

$splat = @{ HomeIp = $HomeIp }
if ($Kill) { $splat.Kill = $true }
& (Join-Path $Phase0Dir 'split-test.ps1') @splat
