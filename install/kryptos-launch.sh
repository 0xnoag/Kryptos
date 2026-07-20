#!/bin/bash

# Kryptos — Web UI Launcher
# Opens the Kryptos web UI in Chromium app mode.
# The daemon must already be running (via systemd).

UI_URL="http://127.0.0.1:8080"

if ! curl -sf "$UI_URL" > /dev/null 2>&1; then
    echo "Kryptos daemon is not running." >&2
    echo "Start it with: sudo systemctl start kryptos.service" >&2
    echo "Or wait a moment if the system just booted." >&2
    exit 1
fi

exec chromium --app="$UI_URL" \
    --window-size=1200,800 \
    --disable-extensions \
    --disable-plugins \
    --no-first-run \
    --user-data-dir=/tmp/kryptos-profile
