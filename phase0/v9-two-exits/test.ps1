# V9 checks, run in a SECOND PowerShell while run.ps1 is running (no admin needed).
# Prints the public IP seen through exit A, exit B and the direct path; they should be three
# different IPs (A's server, B's server, your own network).
#   -ADomain / -BDomain : the test domains given to run.ps1 (defaults match)
#   -Probe http://192.168.1.10,https://192.168.10.5:8006 : also open web pages of devices on each home LAN.
#     (A bare TCP connect is no proof: sing-box's TUN accepts the connection before it reaches the VPN.)
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
    # -k: home devices use self-signed certificates. Any HTTP status (even 401/404) means the device answered.
    $code = & curl.exe -k -s -o NUL --max-time 10 -w '%{http_code}' $p 2>$null
    if ($LASTEXITCODE -eq 0 -and $code -ne '000') { Write-Host "probe ${p}: answered (HTTP $code)" }
    else { Write-Host "probe ${p}: NO answer (curl exit $LASTEXITCODE)" }
}
