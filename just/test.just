set working-directory := '..'

# Run all unit and widget tests (Rust + Flutter).
[default]
all: rust app

# Run Rust workspace tests and doctests.
rust:
    cargo test --workspace --locked

# Run Flutter unit and widget tests.
app:
    cd app && flutter test
