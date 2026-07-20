#!/bin/bash
set -e

# Kryptos — Endpoint Privacy Suite Launcher
# Spawns the daemon as root, then opens the web UI in Chromium app mode as the desktop user.
# Must be run as root (desktop icon wraps with sudo -E).

DAEMON="/usr/local/lib/kryptos/endpoint-privacy-suite"
ENV_FILE="/etc/endpoint-privacy/env"
UI_URL="http://127.0.0.1:8080"
CHROMIUM_PROFILE="/tmp/kryptos-profile"

# Determine the real desktop user (set by sudo -E or logname)
DESKTOP_USER="${SUDO_USER:-$(logname 2>/dev/null || echo kali)}"

if [ "$(id -u)" -ne 0 ]; then
    echo "ERROR: kryptos-launch must be run as root. Use: sudo kryptos-launch" >&2
    exit 1
fi

# Load EPS_PASSWORD from root-only env file
source "$ENV_FILE" 2>/dev/null || { echo "ERROR: Cannot read $ENV_FILE"; exit 1; }
if [ -z "$EPS_PASSWORD" ]; then
    echo "ERROR: EPS_PASSWORD not found in $ENV_FILE" >&2
    echo "Run 'make install' to generate a fresh password." >&2
    exit 1
fi

# Start daemon if not running
if ! pgrep -x "$(basename "$DAEMON")" > /dev/null 2>&1; then
    echo "Starting Kryptos daemon..."
    if [ ! -f "$DAEMON" ]; then
        echo "ERROR: Daemon not found at $DAEMON" >&2
        echo "Run 'make install' from the Kryptos source directory." >&2
        exit 1
    fi
    export EPS_PASSWORD
    "$DAEMON" &
    # Wait for HTTP server (up to 10s)
    for i in $(seq 1 10); do
        if curl -sf "$UI_URL" > /dev/null 2>&1; then
            break
        fi
        sleep 1
    done
fi

echo "Opening Kryptos web UI for user: $DESKTOP_USER"

# Allow desktop user to connect to X server (needed when launched via sudo)
xhost +SI:localuser:"$DESKTOP_USER" 2>/dev/null || true

# Detect X display (fallback to :0)
DISPLAY="${DISPLAY:-:0}"
if [ -z "$XAUTHORITY" ] && [ -f "/home/$DESKTOP_USER/.Xauthority" ]; then
    XAUTHORITY="/home/$DESKTOP_USER/.Xauthority"
fi

# Run chromium as the desktop user (NOT as root — root breaks GPU/X11)
sudo -u "$DESKTOP_USER" DISPLAY="$DISPLAY" XAUTHORITY="$XAUTHORITY" \
    chromium --app="$UI_URL" \
    --window-size=1200,800 \
    --disable-extensions \
    --disable-plugins \
    --no-first-run \
    --user-data-dir="$CHROMIUM_PROFILE" &
