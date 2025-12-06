#!/bin/bash

set -euxo pipefail

# Make sure scratch directory exists
mkdir -p $SCRATCH

# Copy Zsh configuration files to home directory
cp $WORKSPACE/.devcontainer/zsh/.??* /home/ubuntu/

# Ensure Docker socket permissions are correct
sudo chmod 666 /var/run/docker.sock || true

cd $WORKSPACE
