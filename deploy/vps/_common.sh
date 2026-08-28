#!/bin/bash
#
# Shared helpers for the deployment scripts that run FROM your local machine
# over SSH (provision_vps.sh, update_vps.sh). This file is sourced, never
# executed directly, and is deliberately not shipped to the VPS.
#
# Each entrypoint should:
#   cd "$(dirname "$0")"      # so relative paths and ./.playlist.env resolve
#   source ./_common.sh
#   parse_args "$@"           # fills TARGET / SSH_PORT / REMOTE_DIR
#   require_env_file
#   init_ssh
#   ... then call copy_files / start_remote / run_remote as needed.

# --- Connection defaults (an entrypoint may override before parse_args) -------
REMOTE_DIR="playlist"   # relative to the remote home, or an absolute path
SSH_PORT="22"
TARGET=""

# Files shipped to the VPS: the config in the deploy root plus the server-side
# scripts under server/. Deliberately excludes .playlist.env.example, this
# file, and the local-only scripts (provision_vps.sh, update_vps.sh,
# remote_playlist_cli.sh).
DEPLOY_FILES=(
  compose.yaml
  Caddyfile
  .playlist.env
  server/start.sh
  server/stop.sh
  server/install_docker.sh
  server/setup_ufw.sh
)

# Print the calling script's leading comment block (everything after the
# shebang up to the first non-comment line), stripped of the leading "# ".
# Relies on the entrypoint having cd'd into its own directory first, so the
# script is readable by basename regardless of how it was invoked.
usage() {
  awk 'NR>1 && /^#/ { sub(/^#[[:space:]]?/, ""); print; next } NR>1 { exit }' \
      "$(basename "$0")"
}

# parse_args "$@" — fill TARGET / SSH_PORT / REMOTE_DIR from the CLI.
parse_args() {
  while [ $# -gt 0 ]; do
    case "$1" in
      -p) SSH_PORT="$2"; shift 2 ;;
      -d) REMOTE_DIR="$2"; shift 2 ;;
      -h|--help) usage; exit 0 ;;
      -*) echo "Unknown option: $1" >&2; exit 1 ;;
      *)  TARGET="$1"; shift ;;
    esac
  done
  if [ -z "$TARGET" ]; then
    echo "Usage: $0 [user@]HOST [-p SSH_PORT] [-d REMOTE_DIR]" >&2
    exit 1
  fi
}

# Abort early with a clear message if the env file the deployment needs is
# absent (it holds SITE_ADDRESS, DB_NAME, and the music-service credentials).
require_env_file() {
  if [ ! -f .playlist.env ]; then
    echo "ERROR: .playlist.env not found in $(pwd)" >&2
    echo "Copy .playlist.env.example to .playlist.env and fill it in first." >&2
    exit 1
  fi
}

# Populate SSH_OPTS / QDIR for the parsed connection, and define run_remote().
init_ssh() {
  SSH_OPTS=(-o StrictHostKeyChecking=accept-new -p "$SSH_PORT")
  QDIR=$(printf '%q' "$REMOTE_DIR")
}

# Run a command on the VPS, in the deploy dir, with a TTY (so sudo can prompt).
run_remote() { ssh -t "${SSH_OPTS[@]}" "$TARGET" "cd $QDIR && $1"; }

# Confirm the host is reachable before doing anything that assumes it is.
# Echoes the remote uid on stdout so callers can detect root vs. a sudo user;
# returns non-zero (which, under `set -e`, aborts the caller) if unreachable.
connect() {
  local uid
  echo "==> Connecting to $TARGET ..." >&2
  uid=$(ssh "${SSH_OPTS[@]}" "$TARGET" 'id -u') \
    || { echo "ERROR: could not connect to $TARGET" >&2; return 1; }
  printf '%s\n' "$uid"
}

# Copy the config and server scripts to REMOTE_DIR, creating it if needed.
copy_files() {
  echo "==> Copying deployment files to $TARGET:$REMOTE_DIR ..."
  tar -czf - "${DEPLOY_FILES[@]}" \
    | ssh "${SSH_OPTS[@]}" "$TARGET" \
        "mkdir -p $QDIR && tar -C $QDIR -xzf - && chmod +x $QDIR/server/*.sh"
}

# Start (or restart) the stack on the VPS: server/start.sh pulls the latest
# image and brings everything up.
#
# The `sg docker` fallback covers the fresh-provision case: install_docker.sh
# just added the user to the 'docker' group, but that membership isn't active
# in the current SSH session yet, so `sg docker` activates it without an extra
# logout/login round-trip. On an already-provisioned box `docker info` succeeds
# and start.sh runs directly.
start_remote() {
  echo "==> Starting the service ..."
  run_remote "if docker info >/dev/null 2>&1; then ./server/start.sh; else sg docker -c ./server/start.sh; fi"
}
