# Tiny Vid — Linux builder for Tauri v2 (bare build, .deb only).
# Base: Ubuntu 22.04. No GUI runtime. No musl/Alpine.
# Use from repo root: docker build -t tiny-vid-linux-builder -f docker/linux-builder.Dockerfile .

FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

# Tauri v2 Linux prerequisites (https://v2.tauri.app/start/prerequisites/)
RUN apt-get update && apt-get install -y --no-install-recommends \
    libwebkit2gtk-4.1-dev \
    build-essential \
    curl \
    wget \
    file \
    libxdo-dev \
    libssl-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    ca-certificates \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Rust (stable, default)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable \
    && . "$HOME/.cargo/env" && rustup default stable

ENV PATH="/root/.cargo/bin:${PATH}"

# Node 24 (NodeSource)
RUN curl -fsSL https://deb.nodesource.com/setup_24.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

# Yarn (corepack)
RUN corepack enable && corepack prepare yarn@4.12.0 --activate

WORKDIR /app

# Expect repo to be mounted at /app; entrypoint runs build there.
# Default: run shell so CI/scripts can exec specific commands.
CMD ["/bin/bash"]
