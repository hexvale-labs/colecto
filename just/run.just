set working-directory := '..'
j := just_executable() + " --justfile " + quote(justfile_directory() / "justfile")

# Run Colecto in the browser at http://127.0.0.1:4173 (Ctrl+C to stop).
[default]
app:
    {{j}} build rust
    {{j}} build ui
    cargo run -p colecto-host --locked

# Run the Tauri desktop shell (requires platform Tauri build prerequisites).
native:
    {{j}} build ui
    cargo run -p colecto-host --features native --locked

# Run the standalone mail service under any supervisor.
daemon:
    {{j}} build rust
    {{j}} build ui
    cargo run -p colecto-host --bin colecto-daemon --locked

# Run the optional Linux desktop notification agent for the configured HTTPS daemon.
notify:
    {{j}} build rust
    cargo run -p colecto-host --bin colecto-daemon --locked -- notify
