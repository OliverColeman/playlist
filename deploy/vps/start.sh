#!/bin/bash

# Start the deployment in the background.

set -euo pipefail

# Run from this script's directory so relative paths resolve regardless of cwd.
cd "$(dirname "$0")"

COMPOSE=(docker compose --env-file .playlist.env)

# Stop any existing services before (re)starting them.
./stop.sh

echo "Pulling latest image..."
"${COMPOSE[@]}" pull

echo "Starting Docker Compose services..."
"${COMPOSE[@]}" up -d

echo "Playlist and supporting services started successfully"
echo "To stop services: ./stop.sh"
