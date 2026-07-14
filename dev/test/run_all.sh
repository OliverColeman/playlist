#!/usr/bin/env bash
# Runs every test layer in sequence: unit, database integration, then end-to-end.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

"$SCRIPT_DIR/run_unit.sh"
"$SCRIPT_DIR/run_integration.sh"
"$SCRIPT_DIR/run_e2e.sh"
