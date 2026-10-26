# V8 cleanup (Windows side): stop sing-box, remove SplitGate-* adapters and their routes, restore DNS.
# Run in an ADMINISTRATOR PowerShell. Same as phase0\cleanup-all.ps1 -Apply. The Pangolin app is not touched.
# Server side (the proxy on the VM): server\uninstall-socks.sh -y
& (Join-Path $PSScriptRoot '..\cleanup-all.ps1') -Apply
