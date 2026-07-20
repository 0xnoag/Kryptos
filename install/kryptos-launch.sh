#!/bin/bash
set -e

# Kryptos — Endpoint Privacy Suite Launcher
# Spawns the daemon if not running, then opens the web UI in Chromium app mode.

DAEMON="/usr/local/lib/kryptos/endpoint-privacy-suite"
ENV_FILE="/etc/endpoint-privacy/env"
UI_URL="http://127.0.0.1:8080"
CHROMIUM_PROFILE="/tmp/kryptos-profile"

# Load EPS_PASSWORD from root-only env file
export EPS_PASSWORD=$(sudo cat "$ENV_FILE" 2>/dev/null | grep '^EPS_PASSWORD=' | cut -d'"' -f2)

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

# Build chromium args
CHROMIUM_ARGS=(
    --app="$UI_URL"
    --window-size=1200,800
    --disable-extensions
    --disable-plugins
    --no-first-run
    --user-data-dir="$CHROMIUM_PROFILE"
)

# Chromium refuses --no-sandbox as non-root; root requires it
if [ "$(id -u)" -eq 0 ]; then
    CHROMIUM_ARGS+=(--no-sandbox)
fi

exec chromium "${CHROMIUM_ARGS[@]}"
