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

# Install frontend (built by Makefile before install)
if [ -d "$REPO_DIR/dist" ]; then
    echo "Installing web UI -> /opt/kryptos/dist/"
    mkdir -p /opt/kryptos/dist
    cp -r "$REPO_DIR/dist/"* /opt/kryptos/dist/
fi

# Install wrapper script
echo "Installing launcher -> $WRAPPER_DEST"
install -m 755 "$REPO_DIR/install/kryptos-launch.sh" "$WRAPPER_DEST"

# Install desktop entry (system-wide)
echo "Installing desktop entry -> $DESKTOP_DEST"
install -m 644 "$REPO_DIR/install/kryptos.desktop" "$DESKTOP_DEST"

# Create env file with auto-generated password (if not exists)
if [ ! -f "$ENV_DEST" ]; then
    echo
    echo "=== Generating Config Encryption Password ==="
    # Generate a strong random 48-char alphanumeric password
    password=$(python3 -c "import secrets,string; print(''.join(secrets.choice(string.ascii_letters+string.digits) for _ in range(48)))")
    echo "export EPS_PASSWORD=\"$password\"" > "$ENV_DEST"
    chmod 600 "$ENV_DEST"
    echo
    echo "  ┌─────────────────────────────────────────────────────────────┐"
    echo "  │  YOUR KRYPTOS CONFIG PASSWORD (save this securely!)        │"
    echo "  │                                                             │"
    printf "  │  %-59s │\n" "$password"
    echo "  │                                                             │"
    echo "  │  Stored in: $ENV_DEST            │"
    echo "  └─────────────────────────────────────────────────────────────┘"
    echo
    echo "This password encrypts your configuration at /etc/endpoint-privacy/"
    echo "The web UI will display it once on first launch."
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

# Copy .desktop to each user's Desktop folder
for user_home in /home/* /root; do
    if [ -d "$user_home/Desktop" ]; then
        cp "$REPO_DIR/install/kryptos.desktop" "$user_home/Desktop/kryptos.desktop"
        chmod +x "$user_home/Desktop/kryptos.desktop"
        chown "$(basename "$user_home"):$(basename "$user_home")" "$user_home/Desktop/kryptos.desktop" 2>/dev/null || true
    fi
done

echo
echo "=== Installation complete ==="
echo "Launch Kryptos from your desktop icon or application menu, or run: kryptos-launch"
echo "Password was saved to $ENV_DEST (root-only)"
echo "Web UI: http://127.0.0.1:8080"
