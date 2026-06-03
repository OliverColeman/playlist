# syntax=docker/dockerfile:1

##############################################################################
# Build stage — compiles the Dioxus fullstack web bundle and the CLI binary.
##############################################################################
FROM rust:1.96-bookworm AS build

# wasm target for the Dioxus client build.
RUN rustup target add wasm32-unknown-unknown

# Dioxus CLI (`dx`), pinned to the version used in development. cargo-binstall
# fetches a prebuilt binary into $CARGO_HOME/bin (already on PATH) instead of
# compiling the CLI from source.
RUN curl -L --proto '=https' --tlsv1.2 -sSf \
        https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash \
    && cargo binstall -y dioxus-cli@0.7.3

WORKDIR /app
COPY . .

# Fullstack web build: produces the server binary + hashed client assets under
#   target/dx/playlist-web/release/web/  ->  playlist-web (binary) + public/
# Fullstack is auto-detected because the `dioxus` dep enables it by default.
RUN dx build -p playlist-web --platform web --release

# The command-line tools (import, dbmigrate) as a standalone native binary.
RUN cargo build -p playlist-cli --release

##############################################################################
# Runtime stage — slim image holding just the two binaries + web assets.
##############################################################################
FROM debian:bookworm-slim AS runtime

# ca-certificates: TLS to MongoDB / Spotify. libssl3: reqwest's native-tls
# backend used by the CLI's HTTP client.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Web server binary + its `public/` asset directory, kept in the same layout
# the server expects (binary and public/ side by side).
COPY --from=build /app/target/dx/playlist-web/release/web/ ./

# CLI on PATH so you can `docker compose exec web playlist-cli ...`.
COPY --from=build /app/target/release/playlist-cli /usr/local/bin/playlist-cli

# dioxus::serve reads these; default is 127.0.0.1:8080, which is unreachable
# from outside the container — bind to all interfaces instead.
ENV IP=0.0.0.0
ENV PORT=8080

EXPOSE 8080

CMD ["./playlist-web"]
