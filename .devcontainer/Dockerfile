FROM ubuntu:24.04

ARG TIMEZONE=UTC
ENV TZ=$TIMEZONE

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get upgrade -y
RUN apt-get install -y \
    build-essential \
    ca-certificates \
    curl \
    git \
    gnupg \
    lsb-release \
    sudo \
    tzdata \
    unzip \
    wget
RUN apt-get install -y \
    software-properties-common \
    apt-transport-https
RUN apt-get install -y \
    zsh \
    zsh-antidote
RUN apt-get install -y \
    iproute2

# Set timezone
RUN ln -fs /usr/share/zoneinfo/$TZ /etc/localtime && \
    echo $TZ > /etc/timezone && \
    dpkg-reconfigure -f noninteractive tzdata

# Add sudo support to user
ARG USERNAME=ubuntu
RUN echo $USERNAME ALL=\(root\) NOPASSWD:ALL > /etc/sudoers.d/$USERNAME \
    && chmod 0440 /etc/sudoers.d/$USERNAME
# Add user to docker group to access docker daemon from container
RUN groupadd docker \
    && usermod -aG docker $USERNAME
USER $USERNAME
ENV USER=$USERNAME
ENV HOME=/home/$USERNAME

# Use bash for the shell
# SHELL ["/bin/bash", "-o", "pipefail", "-c"]

# Install Rust and Cargo tools
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="$HOME/.cargo/bin:${PATH}"
RUN cargo install --force cargo-make
RUN cargo install trunk

# Install Node.js
RUN curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
ENV NVM_DIR=$HOME/.nvm
RUN . "$NVM_DIR/nvm.sh" && nvm install 22

# Setup Zsh with Antidote, config files are copied in postCreateCommand in devcontainer.json
ENV ZDOTDIR=$HOME
RUN cp /usr/share/zsh-antidote/antidote.zsh $ZDOTDIR/antidote.zsh
