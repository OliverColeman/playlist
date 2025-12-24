#!/bin/bash

dev/run_local/ensure_external_services_running.sh

set -a
source dev/run_local/.local.env
set +a

# Ensure wasm32 target is installed
if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
    echo "Installing wasm32-unknown-unknown target..."
    rustup target add wasm32-unknown-unknown
fi

cargo leptos watch
