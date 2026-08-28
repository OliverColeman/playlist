#!/bin/bash
#
# Stop the Docker Compose services for the playlist deployment. Run this ON THE
# VPS. Keeps the data volumes.
set -euo pipefail

# compose.yaml and .playlist.env live in the deploy root, one level up from
# this script (which lives in server/). Operate from there so compose finds them.
cd "$(dirname "$0")/.."

echo "Stopping Docker Compose services..."
docker compose --env-file .playlist.env down 2>/dev/null || true
echo "Docker Compose services stopped successfully"
