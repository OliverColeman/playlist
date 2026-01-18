FROM ubuntu:24.04

ARG TIMEZONE=UTC
ENV TZ=$TIMEZONE

ENV DEBIAN_FRONTEND=noninteractive

RUN echo Cache bust 1

RUN apt-get update && apt-get upgrade -y
RUN apt-get install -y \
    apt-transport-https \
    build-essential \
    ca-certificates \
    curl \
    git \
    gnupg \
    iproute2 \
    libssl-dev \
    lsb-release \
    openssl \
    pkg-config \
    software-properties-common \
    sudo \
    tzdata \
    unzip \
    wget \
    zsh \
    zsh-antidote

# Add MongoDB repo and install mongodb utils
RUN curl -fsSL https://www.mongodb.org/static/pgp/server-8.0.asc | \
    gpg -o /usr/share/keyrings/mongodb-server-8.0.gpg \
    --dearmor && \
    echo "deb [ arch=amd64,arm64 signed-by=/usr/share/keyrings/mongodb-server-8.0.gpg ] https://repo.mongodb.org/apt/ubuntu noble/mongodb-org/8.2 multiverse" | \
    tee /etc/apt/sources.list.d/mongodb-org-8.2.list && \
    apt-get update && \
    apt-get install -y --no-install-recommends mongodb-mongosh mongodb-database-tools && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

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

# Install Rust and Cargo tools
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="$HOME/.cargo/bin:${PATH}"
RUN curl -sSL https://dioxus.dev/install.sh | bash -s -- v0.7.2
RUN rustup target add wasm32-unknown-unknown

# Setup Zsh with Antidote, config files are copied in postCreateCommand in devcontainer.json
ENV ZDOTDIR=$HOME
RUN cp /usr/share/zsh-antidote/antidote.zsh $ZDOTDIR/antidote.zsh
