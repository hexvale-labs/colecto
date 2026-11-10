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

# Install Chromium for the browser tests (once per machine).
setup-browser:
    cd ui && bunx playwright install chromium

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
