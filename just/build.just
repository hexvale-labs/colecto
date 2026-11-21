set working-directory := '..'

# Build all Rust crates (debug) and the Flutter Web bundle.
[default]
all: rust web

# Build every Rust workspace crate (debug).
rust:
    cargo build --workspace --locked

# Build optimized Rust binaries (colecto-daemon, colecto-mcp).
release:
    cargo build --workspace --release --locked

# `design`: native (default), colecto, fluent, macos, material or cupertino.
# Build the Flutter Web app (served by colecto-daemon via COLECTO_ASSETS).
web design='native':
    cd app && flutter build web --release --no-web-resources-cdn --dart-define=COLECTO_DESIGN={{design}}

# Build the Flutter Web app targeting WebAssembly.
wasm:
    cd app && flutter build web --wasm --release --no-web-resources-cdn

# Build the in-process FFI shared library (`libcolecto_ffi`).
ffi:
    cargo build --release --locked -p colecto-ffi

# Build the Linux desktop Flutter app (with `libcolecto_ffi` bundled).
linux: ffi
    cd app && flutter build linux --release --dart-define=COLECTO_BACKEND=ffi

# Build the colecto-daemon Docker image (bundles app/build/web when present).
docker tag='colecto-daemon:local':
    @if [ -d app/build/web ]; then \
        docker build --build-context web-assets=app/build/web -t {{tag}} .; \
    else \
        docker build -t {{tag}} .; \
    fi
