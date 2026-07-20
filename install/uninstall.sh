#!/bin/bash
set -euo pipefail

# Kryptos — Endpoint Privacy Suite Uninstaller

echo "=== Kryptos Uninstaller ==="

if [ "$EUID" -ne 0 ]; then
    echo "ERROR: Please run as root (sudo)." >&2
    exit 1
fi

# Kill daemon
echo "Stopping daemon..."
pkill -f endpoint-privacy-suite 2>/dev/null || true

# Remove files
echo "Removing files..."
rm -f /usr/local/lib/kryptos/endpoint-privacy-suite
rm -f /usr/local/bin/kryptos-launch
rm -f /usr/local/share/applications/kryptos.desktop
rm -f /opt/kryptos/icon.svg

# Remove per-user symlinks
for user_home in /home/* /root; do
    if [ -f "$user_home/.local/share/applications/kryptos.desktop" ]; then
        rm -f "$user_home/.local/share/applications/kryptos.desktop"
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
