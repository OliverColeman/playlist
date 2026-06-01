#!/bin/bash

if [ -z "$1" ]; then
  echo "Usage: $0 <playlist URI> [user_id] [--name <name>] [--date <YYYY-MM-DD>]"
  exit 1
fi

dev/run_local/ensure_external_services_running.sh

set -a
source dev/run_local/.local.env
set +a

cargo run -p playlist-cli import "$@"