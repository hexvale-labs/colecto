set working-directory := '../..'

# Build every workspace crate and the bundled UI.
[default]
all: rust ui

# Build the core, extension API and host crates (debug); `native` builds the app.
rust:
    cargo build --locked

# Build the bundled UI for the local Host, including the AI extension's service part.
ui: ai-service
    cd ui && bun run --bun build

# Build the AI extension's service part for wasm32; the UI build ships it with its assets.
ai-service:
    rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown || rustup target add wasm32-unknown-unknown
    cargo build --release --target wasm32-unknown-unknown -p colecto-ai-service --locked

# Build the Tauri desktop executable (debug) without starting it.
native: ui
    cargo build -p colecto-app --locked

# Build optimized desktop, daemon and MCP executables with the release profile.
release: ui
    cargo build -p colecto-app -p colecto-host --release --locked
    @echo "Release binaries: $(cargo metadata --format-version 1 --no-deps | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')/release/"

# Package the example extensions (hello-pane and the Oneko cat) into target/extensions.
extension:
    cargo run -p colecto-host --example build-extension --locked

