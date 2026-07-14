#!/usr/bin/env bash
# Runs the unit-test layer: fast, no external services required.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

cargo test --workspace
cargo test -p playlist-core --features server
cargo test -p playlist-web --features server
