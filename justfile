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
verify: fmt-check check lint test ui-check ui-build

# Install the locked frontend dependencies.
setup-ui:
    npm --prefix ui ci

# Install Chromium for the browser tests (once per machine).
setup-browser:
    cd ui && npx playwright install chromium

# Check Svelte components and TypeScript.
ui-check:
    npm --prefix ui run check

# Build the bundled UI for the local Host.
ui-build:
    npm --prefix ui run build

# Run the local demo at http://127.0.0.1:4173 (Ctrl+C to stop).
demo: build ui-build
    cargo run -p colecto-host --locked

# Verify UI flows and restart persistence with an isolated temporary data directory.
ui-test: build ui-build
    node --test ui/tests/restart.mjs
    npm --prefix ui test
