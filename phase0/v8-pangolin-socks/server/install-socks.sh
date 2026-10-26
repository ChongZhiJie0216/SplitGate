#!/bin/sh
# V8 (server side): SOCKS5 proxy for SplitGate's Pangolin exit, on the VPN-Server VM (where newt runs).
# Installs sing-box 1.14.4 (same version as SplitGate) as a systemd service "splitgate-socks":
# one socks inbound with a username + random password, nothing else. It does NOT change routes,
# DNS, firewall rules or the WireGuard / AnyLink / newt setup; it only opens one listening port.
# Undo everything: sh uninstall-socks.sh
#
# Usage (as root on the VM):
#   sh install-socks.sh                 -> check only: shows what it would do and the VM's IPv4 addresses
#   sh install-socks.sh <listen-ip> -y  -> install; <listen-ip> = the VM's LAN address (not 0.0.0.0)
#   PORT=1080 USERNAME=splitgate        -> optional environment overrides
# The password is printed ONCE at the end: type it into SplitGate / run.ps1, it is not shown again.
# It stays only in /etc/splitgate-socks/config.json (0600, owner splitgate-socks), which the proxy needs.
set -eu

VERSION=1.14.4
PORT=${PORT:-1080}
USERNAME=${USERNAME:-splitgate}
PREFIX=/opt/splitgate-socks
CONF_DIR=/etc/splitgate-socks
UNIT=/etc/systemd/system/splitgate-socks.service

case "$(uname -m)" in
    x86_64) ARCH=amd64 ;;
    aarch64) ARCH=arm64 ;;
    *) echo "[ERROR] unsupported CPU: $(uname -m)"; exit 1 ;;
esac
TARBALL="sing-box-$VERSION-linux-$ARCH.tar.gz"
URL="https://github.com/SagerNet/sing-box/releases/download/v$VERSION/$TARBALL"

LISTEN=${1:-}
APPLY=${2:-}

echo "=== SplitGate SOCKS5 proxy (sing-box $VERSION, $ARCH) ==="
echo "--- IPv4 addresses of this VM ---"
ip -4 -o addr show scope global | awk '{print "  " $2 "  " $4}'
if [ -e "$UNIT" ]; then echo "[note] already installed ($UNIT). Run uninstall-socks.sh first to reinstall."; fi
if command -v ss >/dev/null 2>&1 && ss -ltn | awk '{print $4}' | grep -q ":$PORT\$"; then
    echo "[warn] something already listens on port $PORT:"; ss -ltnp | grep ":$PORT " || true
fi
if command -v ufw >/dev/null 2>&1 && ufw status | grep -q "Status: active"; then
    echo "[note] ufw is active: connections from newt come from this VM itself, so no rule is needed for them."
fi

if [ -z "$LISTEN" ] || [ "$APPLY" != "-y" ]; then
    echo
    echo "[check] would: download $URL"
    echo "        install $PREFIX/sing-box, user splitgate-socks, $CONF_DIR/config.json, $UNIT"
    echo "        listen on <listen-ip>:$PORT, username '$USERNAME', random password"
    echo "To install: sh install-socks.sh <listen-ip> -y"
    exit 0
fi
[ "$(id -u)" = 0 ] || { echo "[ERROR] run as root"; exit 1; }
[ -e "$UNIT" ] && { echo "[ERROR] already installed. Run uninstall-socks.sh first."; exit 1; }
case "$LISTEN" in 0.0.0.0|127.*|"") echo "[ERROR] give the VM's LAN address, not $LISTEN"; exit 1 ;; esac
ip -4 -o addr show | grep -q " $LISTEN/" || { echo "[ERROR] $LISTEN is not an address of this VM"; exit 1; }

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
echo "[1/4] download $TARBALL"
if command -v curl >/dev/null 2>&1; then curl -fsSL -o "$TMP/$TARBALL" "$URL"; else wget -q -O "$TMP/$TARBALL" "$URL"; fi
echo "      SHA256: $(sha256sum "$TMP/$TARBALL" | awk '{print $1}')"
echo "      (compare with the digest on https://github.com/SagerNet/sing-box/releases/tag/v$VERSION )"
tar -xzf "$TMP/$TARBALL" -C "$TMP"

echo "[2/4] install"
id splitgate-socks >/dev/null 2>&1 || useradd --system --no-create-home --shell /usr/sbin/nologin splitgate-socks
mkdir -p "$PREFIX" "$CONF_DIR"
install -m 0755 "$TMP/sing-box-$VERSION-linux-$ARCH/sing-box" "$PREFIX/sing-box"
"$PREFIX/sing-box" version | head -n 1

echo "[3/4] config (password generated here, never on the command line)"
PASSWORD=$(head -c 18 /dev/urandom | base64 | tr -d '/+=' | head -c 20)
umask 077
cat > "$CONF_DIR/config.json" <<CONF
{
  "log": { "level": "warn" },
  "inbounds": [
    { "type": "socks", "tag": "socks-in", "listen": "$LISTEN", "listen_port": $PORT,
      "users": [ { "username": "$USERNAME", "password": "$PASSWORD" } ] }
  ],
  "outbounds": [ { "type": "direct", "tag": "direct" } ]
}
CONF
chown -R splitgate-socks:splitgate-socks "$CONF_DIR"
chmod 0600 "$CONF_DIR/config.json"
"$PREFIX/sing-box" check -c "$CONF_DIR/config.json"

echo "[4/4] systemd service"
cat > "$UNIT" <<UNITFILE
[Unit]
Description=SOCKS5 proxy for SplitGate (sing-box $VERSION)
After=network-online.target
Wants=network-online.target

[Service]
User=splitgate-socks
ExecStart=$PREFIX/sing-box run -c $CONF_DIR/config.json
Restart=on-failure
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes

[Install]
WantedBy=multi-user.target
UNITFILE
systemctl daemon-reload
systemctl enable --now splitgate-socks
sleep 1
systemctl --no-pager --lines=5 status splitgate-socks || true

echo
echo "[ok] SOCKS5 proxy on $LISTEN:$PORT"
echo "     username: $USERNAME"
echo "     password: $PASSWORD"
echo "     (shown only now; to get a new one: uninstall-socks.sh, then install again)"
