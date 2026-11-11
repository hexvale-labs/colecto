set working-directory := '..'
j := just_executable() + " --justfile " + quote(justfile_directory() / "justfile")

# Run the fast suites: Rust tests and UI unit tests.
[default]
all: rust ui

# Run workspace tests and doctests.
rust:
    cargo test --workspace --locked

# Run UI unit tests without starting a browser or mail service.
ui:
    bun test ./ui/tests/api.test.mjs ./ui/tests/reply.test.mjs ./ui/tests/when.test.mjs ./ui/tests/layout.test.mjs ./ui/tests/navigation.test.mjs

# Verify UI flows and restart persistence in browsers, with an isolated temporary data directory.
browser:
    {{j}} build rust
    {{j}} build ui
    {{j}} build extension
    bun test ./ui/tests/restart.test.mjs
    cd ui && bun run --bun test

# Linux/X11 native launch smoke check (requires xvfb and xwininfo).
native:
    {{j}} build native
    xvfb-run -a bun test ./ui/tests/native.test.mjs

# Test the Linux resource recorder's aggregation, isolation, and comparison rules.
perf:
    python3 -m unittest discover -s scripts -p '*_test.py'
