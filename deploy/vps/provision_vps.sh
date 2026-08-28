#!/bin/bash
#
# Provision a FRESH VPS to run the Just Dance Archives service, from a LOCAL
# machine over SSH. It copies this directory's deployment files to the server,
# then installs Docker, configures the firewall, ensures swap, and starts the
# stack (pulling the prebuilt image from GHCR).
#
# For a box that is already provisioned, use update_vps.sh instead — it skips
# the one-time host setup and just re-copies config, pulls the latest image,
# and restarts.
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
source ./_common.sh

parse_args "$@"
require_env_file
init_ssh

# --- Connect & detect privilege ----------------------------------------------
REMOTE_UID=$(connect)
if [ "$REMOTE_UID" = "0" ]; then SUDO=""; else SUDO="sudo"; fi

# --- Copy files ---------------------------------------------------------------
copy_files

# --- Install Docker + firewall ------------------------------------------------
echo "==> Installing Docker (may prompt for your sudo password) ..."
run_remote "$SUDO ./server/install_docker.sh"

echo "==> Configuring firewall (SSH, 80, 443) ..."
run_remote "$SUDO ./server/setup_ufw.sh"

# --- Ensure swap (best effort; recommended on a 1 GB box) ---------------------
echo "==> Ensuring a swapfile exists ..."
run_remote "if $SUDO swapon --show | grep -q .; then echo 'Swap already present, skipping.'; else $SUDO fallocate -l 1G /swapfile && $SUDO chmod 600 /swapfile && $SUDO mkswap /swapfile && $SUDO swapon /swapfile && echo '/swapfile none swap sw 0 0' | $SUDO tee -a /etc/fstab >/dev/null && echo 'Created 1G swapfile.'; fi || echo 'WARN: swap setup failed, continuing.'"

# --- Start the stack ----------------------------------------------------------
start_remote

# --- Done ---------------------------------------------------------------------
SITE=$(grep -E '^SITE_ADDRESS=' .playlist.env | cut -d= -f2-)
# Reproduce any non-default connection options so the redeploy hint is copy-paste ready.
HINT=""
[ "$SSH_PORT" != "22" ] && HINT+=" -p $SSH_PORT"
[ "$REMOTE_DIR" != "playlist" ] && HINT+=" -d $REMOTE_DIR"
cat <<DONE

Done — the stack is starting on $TARGET (in $REMOTE_DIR).

Once DNS for ${SITE:-your SITE_ADDRESS} points at this VPS, Caddy will obtain a
TLS certificate automatically and the site will be live at:

    https://${SITE:-<your SITE_ADDRESS>}/

To redeploy after new code lands on main (a new image is published), run from
your local machine:
    ./update_vps.sh $TARGET$HINT

Follow-ups (run on the VPS, from $REMOTE_DIR):
    docker compose --env-file .playlist.env logs -f web        # watch logs
    docker compose --env-file .playlist.env exec web playlist-cli dbmigrate
    ./server/start.sh    # re-pull latest image and restart
    ./server/stop.sh     # stop
DONE
