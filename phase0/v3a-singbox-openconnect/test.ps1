# V3a checks, run in a SECOND Administrator PowerShell while run.ps1 is running.
#   -HomeIp <ip> : expected public IP of the VPN exit (to print PASS/FAIL)
#   -Kill        : also force-kill sing-box and measure how long until the network works again
param([string]$HomeIp = '', [switch]$Kill)
& (Join-Path $PSScriptRoot '..\split-test.ps1') -HomeIp $HomeIp -Kill:$Kill
