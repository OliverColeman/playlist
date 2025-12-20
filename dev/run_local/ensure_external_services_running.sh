#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="$SCRIPT_DIR/external_services.docker_compose.yaml"

echo "Checking if Docker Compose services are running"
RUNNING_SERVICES=$(docker-compose -f "$COMPOSE_FILE" ps --services --filter "status=running" | sort)
EXPECTED_SERVICES=$(docker-compose -f "$COMPOSE_FILE" config --services | sort)

if [ "$RUNNING_SERVICES" = "$EXPECTED_SERVICES" ]; then
    echo "All services are running"
    docker-compose -f "$COMPOSE_FILE" ps
else
    echo "Some services are not running. Starting Docker Compose services..."
    docker-compose -f "$COMPOSE_FILE" up -d

    RUNNING_SERVICES=$(docker-compose -f "$COMPOSE_FILE" ps --services --filter "status=running" | sort)
    
    if [ "$RUNNING_SERVICES" = "$EXPECTED_SERVICES" ]; then
        echo "All services started successfully"
    else
        echo "Failed to start all services. Aborting."
        exit 1
    fi
fi
