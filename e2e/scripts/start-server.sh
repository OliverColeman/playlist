#!/usr/bin/env bash
# Starts the pre-built playlist-web fullstack server for the e2e suite.
# The binary must be run with its own directory as cwd (it serves ./public).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SERVER_DIR="$REPO_ROOT/target/dx/playlist-web/debug/web"

if [ ! -x "$SERVER_DIR/playlist-web" ]; then
    echo "Server binary not found at $SERVER_DIR/playlist-web." >&2
    echo "Build it with: dx build -p playlist-web --fullstack" >&2
    exit 1
fi

# When launched by Playwright, all of these are injected explicitly via the webServer
# `env` block in playwright.config.ts (which wins over ambient variables); the defaults
# below only apply when this script is run by hand.
export IP="${IP:-127.0.0.1}"
export PORT="${PORT:-8811}"
export DB_CONNECTION_STRING="${DB_CONNECTION_STRING:-mongodb://localhost:27017}"
export DB_NAME="${DB_NAME:-playlist_e2e}"
# The app renders dates in the server's local timezone; pin it so rendered dates are
# deterministic (the browser side is pinned via timezoneId in playwright.config.ts).
export TZ="${TZ:-UTC}"

cd "$SERVER_DIR"
exec ./playlist-web
