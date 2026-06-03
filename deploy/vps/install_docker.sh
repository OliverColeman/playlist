#!/bin/bash

set -e

# Install Docker Engine and Docker Compose plugin on Ubuntu/Debian,
# then configure the current user to run Docker without sudo.

if [ "$EUID" -ne 0 ]; then
    echo "Please run as root (sudo ./install_docker.sh)"
    exit 1
fi

REAL_USER="${SUDO_USER:-$USER}"

echo "Installing Docker..."

apt-get update
apt-get install -y ca-certificates curl
install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc
chmod a+r /etc/apt/keyrings/docker.asc

echo \
  "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/ubuntu \
  $(. /etc/os-release && echo "$VERSION_CODENAME") stable" \
  > /etc/apt/sources.list.d/docker.list

apt-get update
apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin

systemctl enable --now docker

echo "Adding $REAL_USER to the docker group..."
usermod -aG docker "$REAL_USER"

echo ""
echo "Docker installed successfully."
echo "Log out and back in (or run 'newgrp docker') for the group change to take effect, then run ./start.sh"
