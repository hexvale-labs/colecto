set working-directory := '..'

# Install everything a fresh checkout needs, except browsers for UI tests.
[default]
all: rust ui

# Fetch the locked Rust dependencies.
rust:
    cargo fetch --locked

# Install the locked frontend dependencies.
ui:
    cd ui && bun install --frozen-lockfile

# Install Chromium and WebKit for the browser tests (once per machine; WebKit also needs system libraries).
browsers:
    cd ui && bunx playwright install --with-deps chromium webkit
