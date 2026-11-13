set working-directory := '../..'

# Run every static check: formatting, compilation, Clippy and Svelte.
[default]
all: fmt rust lint ui

# Compile the core, extension API and host crates; `native` checks the app.
rust:
    cargo check --all-targets --locked

# Check Svelte components and TypeScript.
ui:
    cd ui && bun run --bun check

# Compile the optional native adapter.
native:
    cargo check -p colecto-app --all-targets --locked

# Check Rust formatting without modifying files.
fmt:
    cargo fmt --all -- --check
    cargo fmt --manifest-path docs/extensions/examples/oneko/Cargo.toml -- --check

# Treat Clippy warnings as errors.
lint:
    cargo clippy --all-targets --locked -- -D warnings
    cargo clippy --manifest-path docs/extensions/examples/oneko/Cargo.toml --all-targets --locked -- -D warnings
