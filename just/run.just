set working-directory := '..'
j := just_executable() + " --justfile " + quote(justfile_directory() / "justfile")

# Build Flutter Web + colecto-daemon and serve both at http://127.0.0.1:4173 (Ctrl+C to stop).
[default]
web:
    {{j}} build rust
    {{j}} build web
    COLECTO_ASSETS=app/build/web cargo run -p colecto-daemon --locked

# Run colecto-daemon on 127.0.0.1:4173 (API/SSE only, unless COLECTO_ASSETS is set).
daemon:
    cargo run -p colecto-daemon --locked

# `design` previews another platform's design anywhere:
# native (default), colecto, fluent, macos, material or cupertino.
# `data=worst` swaps in worst-case demo mail to stress the UI.
# Run the Flutter app in built-in Demo mode on the given device (default: linux).
demo device='linux' design='native' data='demo':
    cd app && flutter run -d {{device}} --dart-define=COLECTO_BACKEND=demo \
        --dart-define=COLECTO_DESIGN={{design}} --dart-define=COLECTO_DEMO={{data}}

# Run the Flutter app against a running colecto-daemon.
app device='linux' url='http://127.0.0.1:4173' token='' design='native':
    cd app && flutter run -d {{device}} \
        --dart-define=COLECTO_BACKEND=remote \
        --dart-define=COLECTO_DESIGN={{design}} \
        --dart-define=COLECTO_DAEMON_URL={{url}} \
        --dart-define=COLECTO_TOKEN={{token}}

# Run the stdio MCP bridge (requires COLECTO_MCP_KEY or COLECTO_MCP_KEY_FILE).
mcp:
    cargo run -p colecto-daemon --bin colecto-mcp --locked
