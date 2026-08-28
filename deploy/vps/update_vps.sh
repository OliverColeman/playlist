#!/bin/bash
#
# Update an ALREADY-PROVISIONED VPS from a LOCAL machine over SSH: re-copy the
# deployment config and server scripts, pull the latest image from GHCR, and
# restart the stack. Use this for routine redeploys after new code has landed
# on main (which publishes a new image) or after editing compose.yaml, the
# Caddyfile, or .playlist.env.
#
# It does NOT do first-run host setup (Docker install, firewall, swap) — for a
# fresh box, run provision_vps.sh first.
#
# Usage:
#   ./update_vps.sh [user@]HOST [-p SSH_PORT] [-d REMOTE_DIR]
#
# Examples:
#   ./update_vps.sh root@203.0.113.10
#   ./update_vps.sh deploy@vps.example.com -p 2222 -d /opt/playlist
#
# Requirements:
#   - Local: ssh and tar (both standard on macOS/Linux).
#   - Pass the same [user@]HOST, -p, and -d you gave provision_vps.sh.
#   - ./.playlist.env must be present (copied from .playlist.env.example).

set -euo pipefail

# Operate from this script's own directory so the file list resolves.
cd "$(dirname "$0")"
source ./_common.sh

parse_args "$@"
require_env_file
init_ssh

# Confirm the host is reachable (uid is unused here, but this surfaces a clear
# error before we start shipping files).
connect >/dev/null

copy_files
start_remote

echo
echo "Done — redeployed to $TARGET (in $REMOTE_DIR)."
echo "Watch it come up with:"
echo "    docker compose --env-file .playlist.env logs -f web   # on the VPS, from $REMOTE_DIR"
