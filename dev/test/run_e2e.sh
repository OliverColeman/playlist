#!/usr/bin/env bash
# Runs the Playwright end-to-end suite in e2e/ against the pre-built fullstack
# server binary and a local MongoDB (database: playlist_e2e, dropped and
# re-seeded by the suite's global setup).
#
# Extra arguments are forwarded to `npx playwright test`, e.g.:
#   dev/test/run_e2e.sh tests/search.spec.ts --headed
#
# Environment knobs:
#   SKIP_BUILD=1  use the existing server binary even if sources are newer than it.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

# The e2e suite must only ever touch its reserved database. Export the values
# explicitly so ambient DB_* variables (e.g. pointing at a real deployment) can
# never leak into the seeding step or the server under test. As a second line of
# defence, e2e/db-config.ts refuses any database name that does not start with
# "playlist_e2e".
export DB_CONNECTION_STRING="mongodb://localhost:27017"
export DB_NAME="playlist_e2e"

# Ensure MongoDB is reachable. Probe the port first so this also works where MongoDB
# already runs natively on 27017 (e.g. CI service containers) and Docker is not
# available; only fall back to Docker Compose when nothing is listening.
if timeout 2 bash -c '</dev/tcp/localhost/27017' 2>/dev/null; then
    echo "MongoDB already listening on localhost:27017; not starting Docker Compose."
else
    COMPOSE_FILE="$REPO_ROOT/dev/run_local/external_services.docker_compose.yaml"
    docker compose -f "$COMPOSE_FILE" up -d --wait
fi

# Ensure the fullstack server binary exists and is not older than the sources it is
# built from — a stale binary would silently test old code. (Playwright starts/stops
# the server itself via e2e/scripts/start-server.sh.)
SERVER_BIN="$REPO_ROOT/target/dx/playlist-web/debug/web/playlist-web"
if [ "${SKIP_BUILD:-0}" = "1" ]; then
    echo "SKIP_BUILD=1: skipping the server build/freshness check."
    if [ ! -x "$SERVER_BIN" ]; then
        echo "Server binary not found at $SERVER_BIN; cannot skip the build." >&2
        exit 1
    fi
elif [ ! -x "$SERVER_BIN" ]; then
    echo "Server binary not found; building with dx..."
    dx build -p playlist-web --fullstack
else
    STALE_SOURCE="$(find "$REPO_ROOT/crates/web/src" "$REPO_ROOT/crates/core/src" \
        "$REPO_ROOT/crates/web/assets" -type f -newer "$SERVER_BIN" -print -quit \
        2>/dev/null || true)"
    if [ -n "$STALE_SOURCE" ]; then
        echo "Rebuilding server binary: $STALE_SOURCE is newer than the binary" \
            "(set SKIP_BUILD=1 to skip)."
        dx build -p playlist-web --fullstack
    fi
fi

cd "$REPO_ROOT/e2e"
if [ ! -d node_modules ]; then
    npm install
fi
# Make sure the Playwright browser is present (fresh clones need this; it is a cheap
# no-op when the browser is already installed).
npx playwright install chromium

npx playwright test "$@"
