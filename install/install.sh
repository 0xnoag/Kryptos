#!/bin/bash
set -euo pipefail

# Kryptos — Endpoint Privacy Suite Installer
# Installs: daemon binary, wrapper script, desktop entry, icon, env file.
# Run from the repo root: sudo bash install/install.sh

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BINARY="$REPO_DIR/src-tauri/target/release/endpoint-privacy-suite"
ICON="$REPO_DIR/assets/icon.svg"

# --- Configuration ---
DAEMON_DEST="/usr/local/lib/kryptos/endpoint-privacy-suite"
WRAPPER_DEST="/usr/local/bin/kryptos-launch"
DESKTOP_DEST="/usr/local/share/applications/kryptos.desktop"
ICON_DEST="/opt/kryptos/icon.svg"
ENV_DEST="/etc/endpoint-privacy/env"
CONFIG_DIR="/etc/endpoint-privacy"

echo "=== Kryptos Installer ==="

# Check for root
if [ "$EUID" -ne 0 ]; then
    echo "ERROR: Please run as root (sudo)." >&2
    exit 1
fi

# Check prerequisites
for cmd in chromium curl; do
    if ! command -v "$cmd" > /dev/null 2>&1; then
        echo "WARNING: '$cmd' not found. Install it for full functionality."
    fi
done

# Build daemon if not exists
if [ ! -f "$BINARY" ]; then
    echo "Building daemon..."
    (cd "$REPO_DIR/src-tauri" && cargo build --release)
    if [ ! -f "$BINARY" ]; then
        echo "ERROR: Build failed." >&2
        exit 1
    fi
fi

# Create directories
mkdir -p "$(dirname "$DAEMON_DEST")"
mkdir -p "$(dirname "$WRAPPER_DEST")"
mkdir -p "$(dirname "$DESKTOP_DEST")"
mkdir -p "$(dirname "$ICON_DEST")"
mkdir -p "$CONFIG_DIR"

# Install daemon binary
echo "Installing daemon -> $DAEMON_DEST"
install -m 755 "$BINARY" "$DAEMON_DEST"

# Install icon
echo "Installing icon -> $ICON_DEST"
install -m 644 "$ICON" "$ICON_DEST"

# Install wrapper script
echo "Installing launcher -> $WRAPPER_DEST"
install -m 755 "$REPO_DIR/install/kryptos-launch.sh" "$WRAPPER_DEST"

# Install desktop entry (system-wide)
echo "Installing desktop entry -> $DESKTOP_DEST"
install -m 644 "$REPO_DIR/install/kryptos.desktop" "$DESKTOP_DEST"

# Create env file (if not exists, prompt for password)
if [ ! -f "$ENV_DEST" ]; then
    echo
    echo "=== Config Encryption Password ==="
    echo "This password decrypts your Kryptos configuration."
    read -r -s -p "Enter EPS_PASSWORD: " password
    echo
    read -r -s -p "Confirm password: " password2
    echo
    if [ "$password" != "$password2" ]; then
        echo "ERROR: Passwords do not match." >&2
        exit 1
    fi
    echo "EPS_PASSWORD=\"$password\"" > "$ENV_DEST"
    chmod 600 "$ENV_DEST"
    echo "Password saved to $ENV_DEST (root-only)"
fi

# Symlink desktop entry for all existing users
for user_home in /home/*; do
    user=$(basename "$user_home")
    user_desktop="$user_home/.local/share/applications"
    if [ -d "$user_desktop" ]; then
        ln -sf "$DESKTOP_DEST" "$user_desktop/kryptos.desktop"
    fi
done

# Also for root
if [ -d "/root/.local/share/applications" ]; then
    ln -sf "$DESKTOP_DEST" "/root/.local/share/applications/kryptos.desktop"
fi

echo
echo "=== Installation complete ==="
echo "Launch Kryptos from your application menu or run: kryptos-launch"
echo "Web UI: http://127.0.0.1:8080"
echo "IPC socket: /run/endpoint-privacy/ipc.sock"
