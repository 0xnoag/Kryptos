#!/bin/bash
set -euo pipefail

# Kryptos — Endpoint Privacy Suite Uninstaller

echo "=== Kryptos Uninstaller ==="

if [ "$EUID" -ne 0 ]; then
    echo "ERROR: Please run as root (sudo)." >&2
    exit 1
fi

# Stop and disable systemd service
echo "Stopping systemd service..."
systemctl stop kryptos.service 2>/dev/null || true
systemctl disable kryptos.service 2>/dev/null || true
rm -f /etc/systemd/system/kryptos.service
systemctl daemon-reload

# Remove files
echo "Removing files..."
rm -f /usr/local/lib/kryptos/endpoint-privacy-suite
rm -f /usr/local/bin/kryptos
rm -f /usr/local/bin/kryptos-launch
rm -f /usr/local/share/applications/kryptos.desktop
rm -f /opt/kryptos/icon.svg
rm -rf /opt/kryptos/dist

# Remove per-user files
for user_home in /home/* /root; do
    if [ -d "$user_home" ]; then
        rm -f "$user_home/.local/share/applications/kryptos.desktop"
        rm -f "$user_home/Desktop/kryptos.desktop"
    fi
done

# Ask about config
echo
read -r -p "Remove configuration and encrypted password? (y/N): " confirm
if [ "$confirm" = "y" ] || [ "$confirm" = "Y" ]; then
    rm -rf /etc/endpoint-privacy
    echo "Configuration removed."
else
    echo "Configuration kept at /etc/endpoint-privacy"
fi

echo "=== Uninstall complete ==="
