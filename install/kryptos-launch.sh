#!/bin/bash
set -e

# Kryptos — Endpoint Privacy Suite Launcher
# Spawns the daemon if not running, then opens the web UI in Chromium app mode.

DAEMON="/usr/local/lib/kryptos/endpoint-privacy-suite"
ENV_FILE="/etc/endpoint-privacy/env"
CONFIG_DIR="/etc/endpoint-privacy"
UI_URL="http://127.0.0.1:8080"
CHROMIUM_PROFILE="/tmp/kryptos-profile"

# Load daemon environment (EPS_PASSWORD, etc.)
if [ -f "$ENV_FILE" ]; then
    source "$ENV_FILE"
fi

# Start daemon if not running
if ! pgrep -x "$(basename "$DAEMON")" > /dev/null 2>&1; then
    echo "Starting Kryptos daemon..."
    if [ ! -f "$DAEMON" ]; then
        echo "ERROR: Daemon not found at $DAEMON" >&2
        echo "Run 'sudo make install' from the Kryptos source directory." >&2
        exit 1
    fi
    sudo -E "$DAEMON" &
    # Wait for HTTP server (up to 10s)
    for i in $(seq 1 10); do
        if curl -sf "$UI_URL" > /dev/null 2>&1; then
            break
        fi
        sleep 1
    done
fi

# Launch Chromium in app mode
exec chromium --app="$UI_URL" \
    --window-size=1200,800 \
    --disable-extensions \
    --disable-plugins \
    --no-first-run \
    --user-data-dir="$CHROMIUM_PROFILE"
