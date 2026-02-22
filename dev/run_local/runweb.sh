#!/bin/bash

dev/run_local/ensure_external_services_running.sh

set -a
source dev/run_local/.local.env
set +a

# tailwindcss -i crates/web/tailwind.css -o crates/web/assets/tailwind.css --watch 2>&1 &

dx serve -p playlist-web
