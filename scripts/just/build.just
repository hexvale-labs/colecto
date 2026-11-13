set working-directory := '../..'

# Build every workspace crate and the bundled UI.
[default]
all: rust ui

# Build the core, extension API and host crates (debug); `native` builds the app.
rust:
    cargo build --locked

# Build the bundled UI for the local Host.
ui:
    cd ui && bun run --bun build

# Build the Tauri desktop executable (debug) without starting it.
native: ui
    cargo build -p colecto-app --locked

# Build optimized desktop, daemon and MCP executables with the release profile.
release: ui
    cargo build -p colecto-app -p colecto-host --release --locked
    @echo "Release binaries: $(cargo metadata --format-version 1 --no-deps | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')/release/"

# Build the small installable example extension package from its WAT source.
extension:
    cargo run -p colecto-host --example build-extension --locked

# Build, check and package the third-party pixel cat example with `cargo colecto`.
oneko:
    rustup target add wasm32-unknown-unknown
    cargo run --locked -p cargo-colecto -- build --locked --manifest-path docs/extensions/examples/oneko/Cargo.toml --out docs/extensions/examples/oneko.colectoext
