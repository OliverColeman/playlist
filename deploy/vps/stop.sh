#!/bin/bash
# Stop the Docker Compose services for the playlist deployment.
set -euo pipefail

cd "$(dirname "$0")"

echo "Stopping Docker Compose services..."
docker compose --env-file .playlist.env down 2>/dev/null || true
echo "Docker Compose services stopped successfully"
