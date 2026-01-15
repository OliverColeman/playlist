#!/bin/bash

dev/run_local/ensure_external_services_running.sh

set -a
source dev/run_local/.local.env
set +a

cargo run -p playlist-cli