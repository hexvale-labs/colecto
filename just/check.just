set working-directory := '..'

# Run every static check: formatting, Rust compile/clippy, and Flutter analysis.
[default]
all: fmt rust lint app

# Check Rust and Dart formatting without modifying files.
fmt: fmt-rust fmt-app

# Check Rust formatting without modifying files.
fmt-rust:
    cargo fmt --all -- --check

# Check Dart formatting without modifying files.
fmt-app:
    cd app && dart format --output=none --set-exit-if-changed .

# Compile every workspace crate.
rust:
    cargo check --workspace --all-targets --locked

# Treat Clippy warnings as errors across the workspace.
lint:
    cargo clippy --workspace --all-targets --locked -- -D warnings

# Run Flutter static analysis.
app:
    cd app && flutter analyze
