# V7 cleanup: stop olm / sing-box, remove SplitGate-* adapters and their routes, restore DNS.
# Run in an ADMINISTRATOR PowerShell. Same as phase0\cleanup-all.ps1 -Apply.
& (Join-Path $PSScriptRoot '..\cleanup-all.ps1') -Apply
