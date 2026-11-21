# syntax=docker/dockerfile:1.7

# Optional web UI assets stage.
# By default this is an empty directory so `docker build .` works standalone.
# Pass `--build-context web-assets=app/build/web` (as CI and `just build docker` do)
# to bundle the prebuilt Flutter Web UI into `/srv/colecto/web`.
FROM debian:bookworm-slim AS empty-dir
RUN mkdir -p /empty

FROM scratch AS web-assets
COPY --from=empty-dir /empty /

# Rust toolchain, system packages and cargo-chef. Nothing here depends on the
# source tree or build args, so this layer stays cached until the base image changes.
FROM rust:1-slim-bookworm AS rust-base

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    libdbus-1-dev \
    mold \
    && rm -rf /var/lib/apt/lists/*

ENV CARGO_INCREMENTAL=0 \
    CARGO_TERM_COLOR=always \
    RUSTFLAGS="-C link-arg=-fuse-ld=mold"

RUN cargo install cargo-chef --locked --version 0.1.78 \
    && rm -rf /usr/local/cargo/registry/cache /usr/local/cargo/registry/src

WORKDIR /src

# Reduce the workspace to a dependency recipe (manifests and lockfile only).
FROM rust-base AS rust-planner
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo chef prepare --recipe-path recipe.json

# Build optimized Rust daemon binaries (`colecto-daemon` and `colecto-mcp`).
# Dependencies are compiled in their own layer, keyed only on the recipe, so CI
# restores them from the BuildKit layer cache. BuildKit `--mount=type=cache`
# directories are not exported to GHA or registry caches and start empty on
# every fresh runner, so they are deliberately not used here.
FROM rust-base AS rust-builder

COPY --from=rust-planner /src/recipe.json recipe.json
RUN cargo chef cook --release --locked --package colecto-daemon --recipe-path recipe.json

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Declared last: a changed build arg invalidates every later RUN in the stage, and
# the commit changes on every build. Clean `target/` afterwards so `mode=max` cache
# exports don't upload un-reusable workspace build artifacts on every commit.
ARG COLECTO_COMMIT=
RUN cargo build --package colecto-daemon --release --locked \
    && install -Dm755 target/release/colecto-daemon /out/colecto-daemon \
    && install -Dm755 target/release/colecto-mcp /out/colecto-mcp \
    && rm -rf target

# Minimal non-root runtime image.
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libdbus-1-3 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 --system colecto \
    && useradd --uid 10001 --gid 10001 --system --home-dir /data --shell /usr/sbin/nologin colecto \
    && install -d -o 10001 -g 10001 -m 0700 /data \
    && install -d -o 10001 -g 10001 -m 0755 /srv/colecto/web

COPY --from=rust-builder /out/colecto-daemon /usr/local/bin/colecto-daemon
COPY --from=rust-builder /out/colecto-mcp /usr/local/bin/colecto-mcp
COPY --chmod=755 packaging/docker/entrypoint.sh /usr/local/bin/colecto-entrypoint
COPY --from=web-assets --chown=10001:10001 / /srv/colecto/web/

LABEL org.opencontainers.image.title="Colecto Daemon" \
      org.opencontainers.image.description="Self-hosted Colecto HTTP/SSE/MCP daemon and Flutter Web UI" \
      org.opencontainers.image.source="https://github.com/kogical/colecto" \
      org.opencontainers.image.licenses="MIT OR Apache-2.0"

ENV COLECTO_DATA_DIR=/data \
    COLECTO_BIND=0.0.0.0 \
    COLECTO_PORT=4173 \
    COLECTO_ASSETS=/srv/colecto/web \
    COLECTO_NOTIFICATIONS=off

VOLUME ["/data"]
EXPOSE 4173

USER 10001:10001
WORKDIR /data

ENTRYPOINT ["/usr/local/bin/colecto-entrypoint", "/usr/local/bin/colecto-daemon"]
