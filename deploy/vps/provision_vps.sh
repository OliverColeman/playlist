#!/bin/bash
#
# Provision a VPS to run the Just Dance Archives service from a
# LOCAL machine over SSH. It copies this directory's deployment files to the
# server, then installs Docker, configures the firewall, ensures swap, and
# starts the stack (pulling the prebuilt image from GHCR).
#
# Usage:
#   ./provision_vps.sh [user@]HOST [-p SSH_PORT] [-d REMOTE_DIR]
#
# Examples:
#   ./provision_vps.sh root@203.0.113.10
#   ./provision_vps.sh deploy@vps.example.com -p 2222 -d /opt/playlist
#
# Requirements:
#   - Local: ssh and tar (both standard on macOS/Linux).
#   - The SSH user must be root, or a user with sudo (you may be prompted for
#     its password — a TTY is allocated so the prompt works).
#   - Fill in ./.playlist.env (copy from .playlist.env.example) BEFORE running.
#   - Point your domain's DNS A record (SITE_ADDRESS) at the VPS first, so Caddy
#     can obtain a TLS certificate on startup.

set -euo pipefail

# Operate from this script's own directory so the file list resolves.
cd "$(dirname "$0")"

REMOTE_DIR="playlist"   # relative to the remote home, or an absolute path
SSH_PORT="22"
TARGET=""

while [ $# -gt 0 ]; do
  case "$1" in
    -p) SSH_PORT="$2"; shift 2 ;;
    -d) REMOTE_DIR="$2"; shift 2 ;;
    -h|--help) sed -n '2,21p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "Unknown option: $1" >&2; exit 1 ;;
    *)  TARGET="$1"; shift ;;
  esac
done

if [ -z "$TARGET" ]; then
  echo "Usage: $0 [user@]HOST [-p SSH_PORT] [-d REMOTE_DIR]" >&2
  exit 1
fi

# --- Preflight ----------------------------------------------------------------
if [ ! -f .playlist.env ]; then
  echo "ERROR: .playlist.env not found in $(pwd)" >&2
  echo "Copy .playlist.env.example to .playlist.env and fill it in first." >&2
  exit 1
fi

SSH_OPTS=(-o StrictHostKeyChecking=accept-new -p "$SSH_PORT")
QDIR=$(printf '%q' "$REMOTE_DIR")

# Run a command on the VPS, in the deploy dir, with a TTY (so sudo can prompt).
run_remote() { ssh -t "${SSH_OPTS[@]}" "$TARGET" "cd $QDIR && $1"; }

# Files to ship (deliberately excludes .playlist.env.example and this script).
FILES=(compose.yaml Caddyfile install_docker.sh setup_ufw.sh start.sh stop.sh .playlist.env)

# --- Connect & detect privilege ----------------------------------------------
echo "==> Connecting to $TARGET ..."
REMOTE_UID=$(ssh "${SSH_OPTS[@]}" "$TARGET" 'id -u') \
  || { echo "ERROR: could not connect to $TARGET" >&2; exit 1; }
if [ "$REMOTE_UID" = "0" ]; then SUDO=""; else SUDO="sudo"; fi

# --- Copy files ---------------------------------------------------------------
echo "==> Copying deployment files to $TARGET:$REMOTE_DIR ..."
tar -czf - "${FILES[@]}" \
  | ssh "${SSH_OPTS[@]}" "$TARGET" \
      "mkdir -p $QDIR && tar -C $QDIR -xzf - && chmod +x $QDIR/*.sh"

# --- Install Docker + firewall ------------------------------------------------
echo "==> Installing Docker (may prompt for your sudo password) ..."
run_remote "$SUDO ./install_docker.sh"

echo "==> Configuring firewall (SSH, 80, 443) ..."
run_remote "$SUDO ./setup_ufw.sh"

# --- Ensure swap (best effort; recommended on a 1 GB box) ---------------------
echo "==> Ensuring a swapfile exists ..."
run_remote "if $SUDO swapon --show | grep -q .; then echo 'Swap already present, skipping.'; else $SUDO fallocate -l 1G /swapfile && $SUDO chmod 600 /swapfile && $SUDO mkswap /swapfile && $SUDO swapon /swapfile && echo '/swapfile none swap sw 0 0' | $SUDO tee -a /etc/fstab >/dev/null && echo 'Created 1G swapfile.'; fi || echo 'WARN: swap setup failed, continuing.'"

# --- Start the stack ----------------------------------------------------------
# install_docker.sh just added the user to the 'docker' group; that membership
# isn't active in the current login session yet. `sg docker` activates it so we
# can run without sudo (and without an extra logout/login round-trip).
echo "==> Starting the service ..."
run_remote "if docker info >/dev/null 2>&1; then ./start.sh; else sg docker -c ./start.sh; fi"

# --- Done ---------------------------------------------------------------------
SITE=$(grep -E '^SITE_ADDRESS=' .playlist.env | cut -d= -f2-)
cat <<DONE

Done — the stack is starting on $TARGET (in $REMOTE_DIR).

Once DNS for ${SITE:-your SITE_ADDRESS} points at this VPS, Caddy will obtain a
TLS certificate automatically and the site will be live at:

    https://${SITE:-<your SITE_ADDRESS>}/

Follow-ups (run on the VPS, from $REMOTE_DIR):
    docker compose --env-file .playlist.env logs -f web        # watch logs
    docker compose --env-file .playlist.env exec web playlist-cli dbmigrate
    ./start.sh    # re-pull latest image and restart
    ./stop.sh     # stop
DONE
