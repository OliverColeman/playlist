#!/bin/bash
# UFW firewall configuration for demo web server VM.
# Run as root or with sudo. Safe to re-run — UFW rules are idempotent.

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
  echo "This script must be run as root (sudo $0)" >&2
  exit 1
fi

echo "Configuring UFW..."

# Default policies
ufw default deny incoming
ufw default allow outgoing

# --- Inbound rules ---

# SSH
ufw allow ssh

# Web server
ufw allow 80/tcp
ufw allow 443/tcp

# Ensure loopback traffic is never blocked
ufw allow in on lo
ufw allow out on lo

# Enable (or reload if already enabled)
ufw --force enable

echo ""
ufw status verbose
