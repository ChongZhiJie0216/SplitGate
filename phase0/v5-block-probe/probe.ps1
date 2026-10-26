# V5 probe (READ-ONLY, no admin): which hosts does the school firewall block, and how?
# For every host, connect once from each adapter's IPv4 address (school Ethernet vs phone hotspot Wi-Fi):
#   tls   : TLS handshake result + certificate issuer (a Fortinet issuer = SSL inspection / block page)
#   http  : HTTP status from curl (a block page usually answers 403 or redirects)
# Nothing is changed on the system. sing-box should NOT be running.
param(
    [string[]]$Adapters = @('Ethernet', 'Wi-Fi'),
    [string[]]$Hosts = @(
        # Minecraft / PrismLauncher sign-in chain
        'login.live.com', 'login.microsoftonline.com', 'user.auth.xboxlive.com', 'xsts.auth.xboxlive.com',
        'api.minecraftservices.com', 'sessionserver.mojang.com', 'session.minecraft.net', 'textures.minecraft.net',
        'piston-meta.mojang.com', 'libraries.minecraft.net', 'resources.download.minecraft.net',
        # Steam
        'store.steampowered.com', 'login.steampowered.com', 'api.steampowered.com', 'steamcommunity.com',
        'steamcdn-a.akamaihd.net',
        # Xbox app
        'www.xbox.com', 'title.mgt.xboxlive.com', 'xboxservices.com',
        # controls (expected to work)
        'www.microsoft.com', 'www.google.com'
    ),
    [int]$TimeoutMs = 4000,
    # Probe through the running sing-box TUN instead (no source address binding). Needs sing-box running.
    [switch]$ViaTun
)
. (Join-Path $PSScriptRoot '..\common.ps1')
$ErrorActionPreference = 'Continue'
if ($ViaTun) {
    if (-not (Get-OurProcesses)) { Write-Host '[ERROR] -ViaTun needs sing-box running (phase0\v5-block-probe\run.ps1).'; exit 1 }
} elseif (Get-OurProcesses) { Write-Host '[ERROR] sing-box is running. Stop it first, or use -ViaTun.'; exit 1 }

function Test-Tls([string]$name, [string]$localIp) {
    if ($localIp) { $client = New-Object Net.Sockets.TcpClient([Net.IPEndPoint]::new([Net.IPAddress]::Parse($localIp), 0)) }
    else { $client = New-Object Net.Sockets.TcpClient([Net.Sockets.AddressFamily]::InterNetwork) }
    try {
        $ip = [Net.Dns]::GetHostAddresses($name) | Where-Object { $_.AddressFamily -eq 'InterNetwork' } | Select-Object -First 1
        if (-not $ip) { return 'DNS: no IPv4' }
        $task = $client.ConnectAsync($ip, 443)
        if (-not $task.Wait($TimeoutMs)) { return "TCP timeout ($ip)" }
        $script:issuer = ''
        $cb = [Net.Security.RemoteCertificateValidationCallback] { param($s, $cert, $chain, $err) $script:issuer = $cert.Issuer; $script:sslErr = $err; $true }
        $ssl = New-Object Net.Security.SslStream($client.GetStream(), $false, $cb)
        $client.ReceiveTimeout = $TimeoutMs; $client.SendTimeout = $TimeoutMs
        $ssl.AuthenticateAsClient($name)
        $cn = if ($script:issuer -match 'CN=([^,]+)') { $Matches[1] } else { $script:issuer }
        $o = if ($script:issuer -match 'O=([^,]+)') { $Matches[1] } else { '' }
        $flag = if ($script:sslErr -ne 'None') { " [cert error: $($script:sslErr)]" } else { '' }
        return "OK issuer=$cn / $o$flag"
    } catch {
        $m = $_.Exception
        while ($m.InnerException) { $m = $m.InnerException }
        return "FAIL $($m.GetType().Name): $($m.Message.Split([char]10)[0].Trim())"
    } finally { $client.Close() }
}

function Test-Http([string]$name, [string]$localIp) {
    $bind = @(); if ($localIp) { $bind = @('--interface', $localIp) }
    $out = curl.exe -s -o NUL @bind --max-time ([math]::Ceiling($TimeoutMs / 1000) + 2) -w '%{http_code} %{redirect_url}' "https://$name/" 2>&1
    $code = "$LASTEXITCODE"
    if ($code -ne '0') { return "curl exit $code" }
    return "$out"
}

$ips = [ordered]@{}
if ($ViaTun) { $ips['TUN'] = ''; $Adapters = @() }
foreach ($a in $Adapters) {
    $ip = Get-NetIPAddress -InterfaceAlias $a -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.IPAddress -notlike '169.254.*' } | Select-Object -First 1 -Expand IPAddress
    if ($ip) { $ips[$a] = $ip } else { Write-Host "[skip] adapter '$a' has no IPv4 address" }
}
Write-Host ("adapters: " + (($ips.Keys | ForEach-Object { "$_=$($ips[$_])" }) -join ', '))
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
New-Item -ItemType Directory -Force $LogDir | Out-Null
$csv = Join-Path $LogDir "v5-probe-$stamp.csv"
$rows = foreach ($h in $Hosts) {
    foreach ($a in $ips.Keys) {
        $t = Test-Tls $h $ips[$a]
        $w = Test-Http $h $ips[$a]
        Write-Host ("{0,-34} {1,-9} tls: {2}`n{3,-44} http: {4}" -f $h, $a, $t, '', $w)
        [pscustomobject]@{ host = $h; adapter = $a; tls = $t; http = $w }
    }
}
$rows | Export-Csv -NoTypeInformation -Encoding UTF8 $csv
Write-Host "[saved] $csv"
