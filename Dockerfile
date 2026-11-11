# syntax=docker/dockerfile:1
# Colecto mail daemon. See docs/docker.md for configuration and docker compose usage.

FROM oven/bun:1.4.4 AS ui
WORKDIR /src/ui
COPY ui/package.json ui/bun.lock ./
RUN bun install --frozen-lockfile
COPY ui/ ./
RUN bun run --bun build

FROM rust:1-bookworm AS daemon
RUN apt-get update \
    && apt-get install -y --no-install-recommends libssl-dev libdbus-1-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY ui/src/extensions ui/src/extensions
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p colecto-host --bin colecto-daemon \
    && cp target/release/colecto-daemon /usr/local/bin/colecto-daemon

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 libdbus-1-3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home-dir /data colecto \
    && install -d -m 700 -o colecto -g colecto /data
COPY --from=daemon /usr/local/bin/colecto-daemon /usr/local/bin/colecto-daemon
COPY --from=ui /src/ui/dist /usr/share/colecto/ui
COPY packaging/docker/entrypoint.sh /usr/local/bin/colecto-entrypoint
ENV COLECTO_DATA_DIR=/data \
    COLECTO_TOKEN_FILE=/data/access-token \
    COLECTO_ASSETS=/usr/share/colecto/ui \
    COLECTO_BIND=0.0.0.0 \
    COLECTO_PORT=4173
USER colecto
VOLUME /data
EXPOSE 4173
ENTRYPOINT ["colecto-entrypoint"]
CMD ["colecto-daemon"]
