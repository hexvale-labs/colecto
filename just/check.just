set working-directory := '..'

# Run every static check: formatting, compilation, Clippy and Svelte.
[default]
all: fmt rust lint ui

# Compile every workspace crate.
rust:
    cargo check --workspace --all-targets --locked

# Check Svelte components and TypeScript.
ui:
    cd ui && bun run --bun check

# Compile the optional native adapter.
native:
    cargo check -p colecto-host --features native --all-targets --locked

# Check Rust formatting without modifying files.
fmt:
    cargo fmt --all -- --check

# Treat Clippy warnings as errors.
lint:
    cargo clippy --workspace --all-targets --locked -- -D warnings
