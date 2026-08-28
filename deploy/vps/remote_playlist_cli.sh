#!/bin/bash
#
# Run `playlist-cli` inside the web container on the VPS, from your LOCAL machine.
# It SSHes to the server and runs, in the remote deploy directory:
#     docker compose --env-file .playlist.env exec web playlist-cli <args...>
# forwarding every argument through unchanged.
#
# Usage:
#   ./remote_playlist_cli.sh <playlist-cli args...>
#
# Examples:
#   ./remote_playlist_cli.sh dbmigrate
#   ./remote_playlist_cli.sh import "https://open.spotify.com/playlist/xxxx"
#   ./remote_playlist_cli.sh import "https://tidal.com/browse/playlist/xxxx"
#   ./remote_playlist_cli.sh import <uri> someUserId --name "My List" --date 2026-06-01
#   ./remote_playlist_cli.sh set-compiler-name <compiler_id> "Fitness Marshall"
#
# Connection settings come from env vars (with sensible defaults) so they never
# clash with the arguments forwarded to playlist-cli:
#   PLAYLIST_VPS         [user@]host to SSH to. Defaults to SITE_ADDRESS from
#                        ./.playlist.env (the site's domain resolves to the VPS).
#                        Set this when you need a specific SSH user, e.g.
#                        PLAYLIST_VPS=root@vps.example.com
#   PLAYLIST_SSH_PORT    SSH port (default: 22). Match the -p you gave provision_vps.sh.
#   PLAYLIST_REMOTE_DIR  Deploy dir on the VPS (default: playlist). Match the -d
#                        you gave provision_vps.sh.
#
# Requirements: ssh locally, and a running deployment on the VPS.

set -euo pipefail

# Run from this script's directory so ./.playlist.env resolves regardless of cwd.
cd "$(dirname "$0")"

if [ $# -eq 0 ]; then
  cat >&2 <<'USAGE'
Usage: ./remote_playlist_cli.sh <playlist-cli args...>

Examples:
  ./remote_playlist_cli.sh dbmigrate
  ./remote_playlist_cli.sh import <uri> [user_id] [--name <name>] [--date YYYY-MM-DD]
  ./remote_playlist_cli.sh set-compiler-name <compiler_id> "<name>"

Connection (env vars): PLAYLIST_VPS=[user@]host  PLAYLIST_SSH_PORT=22  PLAYLIST_REMOTE_DIR=playlist
USAGE
  exit 1
fi

# --- Resolve the SSH target ---------------------------------------------------
# Prefer $PLAYLIST_VPS; otherwise fall back to SITE_ADDRESS from .playlist.env.
TARGET="${PLAYLIST_VPS:-}"
if [ -z "$TARGET" ] && [ -f .playlist.env ]; then
  TARGET=$(grep -E '^SITE_ADDRESS=' .playlist.env | cut -d= -f2- || true)
fi
if [ -z "$TARGET" ]; then
  echo "ERROR: no VPS host. Set PLAYLIST_VPS=[user@]host, or SITE_ADDRESS in .playlist.env." >&2
  exit 1
fi

SSH_PORT="${PLAYLIST_SSH_PORT:-22}"
REMOTE_DIR="${PLAYLIST_REMOTE_DIR:-playlist}"
QDIR=$(printf '%q' "$REMOTE_DIR")

# Quote each forwarded arg so it survives the remote shell (e.g. --name "My List").
REMOTE_ARGS=""
for a in "$@"; do
  REMOTE_ARGS+=" $(printf '%q' "$a")"
done

# One argv array (never empty) keeps `set -u` happy on macOS's bash 3.2.
# Allocate a TTY only when we have one locally, so interactive use works but
# piping/scripting doesn't trip over "the input device is not a TTY".
SSH_ARGS=(-o StrictHostKeyChecking=accept-new -p "$SSH_PORT")
if [ -t 0 ]; then
  SSH_ARGS+=(-t); EXEC_T=""
else
  EXEC_T="-T"
fi

REMOTE_CMD="cd $QDIR && docker compose --env-file .playlist.env exec $EXEC_T web playlist-cli$REMOTE_ARGS"

echo "==> $TARGET (port $SSH_PORT, dir $REMOTE_DIR): playlist-cli$REMOTE_ARGS" >&2
exec ssh "${SSH_ARGS[@]}" "$TARGET" "$REMOTE_CMD"
