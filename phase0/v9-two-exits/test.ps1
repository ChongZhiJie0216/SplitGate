# V9 checks, run in a SECOND PowerShell while run.ps1 is running (no admin needed).
# Prints the public IP seen through exit A, exit B and the direct path; they should be three
# different IPs (A's server, B's server, your own network).
#   -ADomain / -BDomain : the test domains given to run.ps1 (defaults match)
#   -Probe 192.168.1.10:8006,192.168.10.5:443 : also try a TCP connection to devices on each home LAN
param([string]$ADomain = 'ifconfig.me', [string]$BDomain = 'api.ipify.org', [string]$DirectUrl = 'https://ipinfo.io/ip', [string[]]$Probe = @())
$Probe = @($Probe | ForEach-Object { "$_" -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
function Get-Ip([string]$url) {
    $out = & curl.exe -s --max-time 15 $url 2>$null
    if ($LASTEXITCODE -ne 0 -or -not $out) { return "(failed, curl exit $LASTEXITCODE)" }
    return ("$out").Trim()
}
$ipA = Get-Ip "https://$ADomain"
$ipB = Get-Ip "https://$BDomain"
$ipD = Get-Ip $DirectUrl
Write-Host "exit A   ($ADomain): $ipA"
Write-Host "exit B   ($BDomain): $ipB"
Write-Host "direct   ($DirectUrl): $ipD"
$ok = ($ipA -notlike '(failed*') -and ($ipB -notlike '(failed*') -and ($ipD -notlike '(failed*') -and ($ipA -ne $ipB) -and ($ipA -ne $ipD) -and ($ipB -ne $ipD)
Write-Host $(if ($ok) { 'PASS: three different IPs' } else { 'CHECK: the three IPs should all differ (see above)' })
foreach ($p in $Probe) {
    $i = $p.LastIndexOf(':'); $h = $p.Substring(0, $i); $port = [int]$p.Substring($i + 1)
    $c = New-Object Net.Sockets.TcpClient
    try {
        $t = $c.BeginConnect($h, $port, $null, $null)
        $r = $t.AsyncWaitHandle.WaitOne(5000) -and $c.Connected
        Write-Host ("probe {0}: {1}" -f $p, $(if ($r) { 'reachable' } else { 'NOT reachable (5 s)' }))
    } catch { Write-Host "probe ${p}: NOT reachable ($($_.Exception.Message))" } finally { $c.Close() }
}
