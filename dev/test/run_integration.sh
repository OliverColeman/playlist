#!/usr/bin/env bash
# Runs the database integration-test layer against a local MongoDB
# (started via Docker Compose if not already running).
#
# Uses the reserved test databases playlist_test_core and playlist_test_cli;
# the test suites drop/clean their own database at start, so runs are idempotent.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

# Ensure MongoDB is reachable. Probe the port first so this also works where MongoDB
# already runs natively on 27017 (e.g. CI service containers) and Docker is not
# available; only fall back to Docker Compose when nothing is listening.
if timeout 2 bash -c '</dev/tcp/localhost/27017' 2>/dev/null; then
    echo "MongoDB already listening on localhost:27017; not starting Docker Compose."
else
    COMPOSE_FILE="$REPO_ROOT/dev/run_local/external_services.docker_compose.yaml"
    docker compose -f "$COMPOSE_FILE" up -d --wait
fi

DB_CONNECTION_STRING="mongodb://localhost:27017" DB_NAME="playlist_test_core" \
    cargo test -p playlist-core --features server --test db_integration -- --include-ignored --test-threads=1

# The core lib also has an #[ignore]-gated test (Config::from_env in
# crates/core/src/config.rs) that mutates the process environment and therefore must
# run single-threaded via this script; --include-ignored picks it up.
DB_CONNECTION_STRING="mongodb://localhost:27017" DB_NAME="playlist_test_core" \
    cargo test -p playlist-core --features server --lib -- --include-ignored --test-threads=1

cargo test -p playlist-cli --test cli_integration -- --include-ignored --test-threads=1
