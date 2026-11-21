# syntax=docker/dockerfile:1.7

# Optional web UI assets stage.
# By default this is an empty directory so `docker build .` works standalone.
# Pass `--build-context web-assets=app/build/web` (as CI and `just build docker` do)
# to bundle the prebuilt Flutter Web UI into `/srv/colecto/web`.
FROM debian:bookworm-slim AS empty-dir
RUN mkdir -p /empty

FROM scratch AS web-assets
COPY --from=empty-dir /empty /

# Build optimized Rust daemon binaries (`colecto-daemon` and `colecto-mcp`).
FROM rust:1-slim-bookworm AS rust-builder

ARG COLECTO_COMMIT=
ENV COLECTO_COMMIT=${COLECTO_COMMIT}

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    libdbus-1-dev \
    mold \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src

# Copy manifests and workspace crates.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

ENV CARGO_INCREMENTAL=0 \
    CARGO_TERM_COLOR=always \
    RUSTFLAGS="-C link-arg=-fuse-ld=mold"

RUN --mount=type=cache,id=colecto-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=colecto-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=colecto-target-release,target=/src/target,sharing=locked \
    cargo build --package colecto-daemon --release --locked \
    && install -Dm755 target/release/colecto-daemon /out/colecto-daemon \
    && install -Dm755 target/release/colecto-mcp /out/colecto-mcp

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
