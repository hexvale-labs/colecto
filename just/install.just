set working-directory := '..'

# Fetch locked Rust and Flutter dependencies.
[default]
all: rust app

# Fetch locked Rust crate dependencies.
rust:
    cargo fetch --locked

# Fetch Flutter/Dart dependencies.
app:
    cd app && flutter pub get
