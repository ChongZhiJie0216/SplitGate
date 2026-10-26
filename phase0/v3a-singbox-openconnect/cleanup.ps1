# V3a cleanup: stop sing-box, remove SplitGate-TUN and its routes, restore DNS.
# Run in an ADMINISTRATOR PowerShell. Same as phase0\cleanup-all.ps1 -Apply.
& (Join-Path $PSScriptRoot '..\cleanup-all.ps1') -Apply
