#!/bin/sh
# V8 (server side) cleanup: remove everything install-socks.sh added. Nothing else is touched.
# Usage (as root on the VM): sh uninstall-socks.sh        (check only)
#                            sh uninstall-socks.sh -y     (really remove)
set -eu
UNIT=/etc/systemd/system/splitgate-socks.service
for p in "$UNIT" /opt/splitgate-socks /etc/splitgate-socks; do [ -e "$p" ] && echo "[found] $p"; done
id splitgate-socks >/dev/null 2>&1 && echo "[found] user splitgate-socks"
if [ "${1:-}" != "-y" ]; then echo "To remove: sh uninstall-socks.sh -y"; exit 0; fi
[ "$(id -u)" = 0 ] || { echo "[ERROR] run as root"; exit 1; }
systemctl disable --now splitgate-socks 2>/dev/null || true
rm -f "$UNIT"
systemctl daemon-reload
rm -rf /opt/splitgate-socks /etc/splitgate-socks
id splitgate-socks >/dev/null 2>&1 && userdel splitgate-socks
echo "[done] removed. Check: systemctl status splitgate-socks (should be 'could not be found')"
