#!/bin/bash
#
# Start (or restart) the deployment in the background. Run this ON THE VPS.
# Pulls the latest image from GHCR, then brings the stack up.

set -euo pipefail

# This script lives in server/; compose.yaml and .playlist.env live in the
# deploy root one level up. Resolve both, then operate from the root so
# `docker compose` picks up compose.yaml and --env-file resolves.
SERVER_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SERVER_DIR/.."

COMPOSE=(docker compose --env-file .playlist.env)

# Stop any existing services before (re)starting them.
"$SERVER_DIR/stop.sh"

echo "Pulling latest image..."
"${COMPOSE[@]}" pull

echo "Starting Docker Compose services..."
"${COMPOSE[@]}" up -d

echo "Playlist and supporting services started successfully"
echo "To stop services: server/stop.sh"
