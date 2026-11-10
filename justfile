# List available operations.
default:
    @just --list

# Compile every workspace crate.
check:
    cargo check --workspace --all-targets --locked

# Build every workspace crate.
build:
    cargo build --workspace --locked

# Run workspace tests and doctests.
test:
    cargo test --workspace --locked

# Format Rust source.
fmt:
    cargo fmt --all

# Check formatting without modifying files.
fmt-check:
    cargo fmt --all -- --check

# Treat Clippy warnings as errors.
lint:
    cargo clippy --workspace --all-targets --locked -- -D warnings

# Run the routine verification suite.
verify: fmt-check check lint test ui-check ui-unit ui-build

# Install the locked frontend dependencies.
setup-ui:
    cd ui && bun install --frozen-lockfile

# Install Chromium and WebKit for the browser tests (once per machine; WebKit also needs system libraries).
setup-browser:
    cd ui && bunx playwright install --with-deps chromium webkit

# Check Svelte components and TypeScript.
ui-check:
    cd ui && bun run --bun check

# Check browser service error handling without starting a browser or mail service.
ui-unit:
    bun test ./ui/tests/api.test.mjs ./ui/tests/reply.test.mjs ./ui/tests/when.test.mjs ./ui/tests/layout.test.mjs

# Build the bundled UI for the local Host.
ui-build:
    cd ui && bun run --bun build

# Run the local demo at http://127.0.0.1:4173 (Ctrl+C to stop).
demo: build ui-build
    cargo run -p colecto-host --locked

# Verify UI flows and restart persistence with an isolated temporary data directory.
ui-test: build ui-build extension-example
    bun test ./ui/tests/restart.test.mjs
    cd ui && bun run --bun test

# Run the Tauri desktop shell (requires platform Tauri build prerequisites).
native: ui-build
    cargo run -p colecto-host --features native --locked

# Build the small installable Wasm extension example from its WAT source.
extension-example:
    cargo run -p colecto-host --example build-extension --locked

# Compile the optional native adapter.
native-check:
    cargo check -p colecto-host --features native --all-targets --locked

# Build the Tauri desktop executable without starting it.
native-build: ui-build
    cargo build -p colecto-host --features native --locked

# Tauri's generator also emits Android, iOS and macOS sets; copy those in when those targets exist.
# Regenerate the desktop app icons from the single source, crates/colecto-host/icons/icon.svg.
icons:
    #!/usr/bin/env bash
    set -euo pipefail
    out=$(mktemp -d); trap 'rm -rf "$out"' EXIT
    bunx --bun @tauri-apps/cli@2.9.4 icon crates/colecto-host/icons/icon.svg -o "$out" > /dev/null
    cp "$out"/{32x32.png,128x128.png,128x128@2x.png,icon.png,icon.ico} crates/colecto-host/icons/

# Install the Linux desktop entry and icons for the native build. Wayland shells take the window icon from here.
desktop-install: native-build
    #!/usr/bin/env bash
    set -euo pipefail
    target=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
    data="${XDG_DATA_HOME:-$HOME/.local/share}"
    for size in 32x32 128x128 256x256; do
      file=crates/colecto-host/icons/$size.png; [ "$size" = 256x256 ] && file=crates/colecto-host/icons/128x128@2x.png
      install -Dm644 "$file" "$data/icons/hicolor/$size/apps/app.colecto.mail.png"
    done
    install -Dm644 crates/colecto-host/icons/icon.svg "$data/icons/hicolor/scalable/apps/app.colecto.mail.svg"
    mkdir -p "$data/applications"
    sed "s|^Exec=.*|Exec=$target/debug/colecto-host|" packaging/linux/app.colecto.mail.desktop > "$data/applications/app.colecto.mail.desktop"
    echo "Installed Colecto desktop entry in $data/applications"

# Linux/X11 native launch smoke check (requires xvfb and xwininfo).
native-test: native-build
    xvfb-run -a bun test ./ui/tests/native.test.mjs

# Test the Linux resource recorder's aggregation, isolation, and comparison rules.
perf-test:
    python3 -m unittest discover -s scripts -p '*_test.py'

# Measure three isolated daemon/browser workloads and retain timestamped history.
perf: build ui-build perf-test
    python3 scripts/resource_bench.py

# Run the normal verification suite and record resource usage for feature changes.
perf-check: verify perf

# Run the standalone mail service under any supervisor.
daemon: build ui-build
    cargo run -p colecto-host --bin colecto-daemon --locked

# Run the optional Linux desktop notification agent for the configured HTTPS daemon.
notify: build
    cargo run -p colecto-host --bin colecto-daemon --locked -- notify
