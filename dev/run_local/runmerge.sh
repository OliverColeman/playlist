#!/bin/bash

if [ -z "$3" ]; then
  echo "Usage: $0 <type> <keep_id> <remove_id> [--dry-run]"
  echo "  <type> is one of: artist, album, track, compiler, playlist"
  echo "  Merges the <remove_id> record into the <keep_id> record."
  exit 1
fi

dev/run_local/ensure_external_services_running.sh

set -a
source dev/run_local/.local.env
set +a

cargo run -p playlist-cli merge "$@"
