#!/bin/sh
# Installs the Sompio Voice server as a service on a Linux VPS (Debian 12, Ubuntu 22.04 or newer).
# Run it from the folder that holds sompio-voice-server:   sudo sh install.sh
# Change the port with:  sudo sh install.sh 9988
set -e
PORT="${1:-9987}"
[ "$(id -u)" = 0 ] || { echo "Run it with sudo: sudo sh install.sh"; exit 1; }
[ -f ./sompio-voice-server ] || { echo "Put sompio-voice-server in this folder first."; exit 1; }

install -m 755 ./sompio-voice-server /usr/local/bin/sompio-voice-server
id sompio-voice >/dev/null 2>&1 || useradd --system --home /var/lib/sompio-voice --shell /usr/sbin/nologin sompio-voice
mkdir -p /var/lib/sompio-voice && chown sompio-voice:sompio-voice /var/lib/sompio-voice && chmod 700 /var/lib/sompio-voice

cat > /etc/systemd/system/sompio-voice.service <<UNIT
[Unit]
Description=Sompio Voice server
After=network-online.target
Wants=network-online.target

[Service]
User=sompio-voice
ExecStart=/usr/local/bin/sompio-voice-server --data /var/lib/sompio-voice --port ${PORT}
Restart=on-failure
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ReadWritePaths=/var/lib/sompio-voice

[Install]
WantedBy=multi-user.target
UNIT

if command -v ufw >/dev/null 2>&1 && ufw status | grep -q active; then ufw allow "${PORT}/udp"; fi
systemctl daemon-reload
systemctl enable --now sompio-voice
sleep 2
echo
journalctl -u sompio-voice --no-pager -n 20 -o cat
echo
echo "Done. It starts by itself after a reboot."
echo "  Status:        systemctl status sompio-voice"
echo "  Server log:    journalctl -u sompio-voice -o cat"
echo "  Set password:  sudo -u sompio-voice sompio-voice-server --data /var/lib/sompio-voice set-password 'secret' && sudo systemctl restart sompio-voice"
echo "  Open UDP port ${PORT} in your provider's firewall too, if it has one."
echo "  Domain: point an A record (e.g. voice.yourdomain.com) at this server's IP; people connect to voice.yourdomain.com"
