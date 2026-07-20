#!/bin/bash
exec chromium --app=http://127.0.0.1:8080 --window-size=1200,800 \
    --disable-extensions --disable-plugins --no-first-run \
    --user-data-dir=/tmp/kryptos-profile
