set working-directory := '..'

# Build every workspace crate and the bundled UI.
[default]
all: rust ui

# Build every workspace crate (debug).
rust:
    cargo build --workspace --locked

# Build the bundled UI for the local Host.
ui:
    cd ui && bun run --bun build

# Build the Tauri desktop executable (debug) without starting it.
native: ui
    cargo build -p colecto-host --features native --locked

# Build optimized desktop, daemon and MCP executables with the release profile.
release: ui
    cargo build -p colecto-host --features native --release --locked
    @echo "Release binaries: $(cargo metadata --format-version 1 --no-deps | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')/release/"

# Build the small installable example extension package from its WAT source.
extension:
    cargo run -p colecto-host --example build-extension --locked
