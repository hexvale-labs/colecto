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
verify: fmt-check check lint test
