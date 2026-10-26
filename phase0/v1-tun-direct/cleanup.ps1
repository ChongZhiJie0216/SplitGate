# V1 cleanup: stop sing-box, remove the SplitGate-TUN adapter and its routes, restore DNS from baseline.
# Run in an ADMINISTRATOR PowerShell. Same as phase0\cleanup-all.ps1 -Apply.
& (Join-Path $PSScriptRoot '..\cleanup-all.ps1') -Apply
