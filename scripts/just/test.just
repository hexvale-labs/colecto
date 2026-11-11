set working-directory := '../..'
j := just_executable() + " --justfile " + quote(justfile_directory() / "justfile")

# Run the fast suites: Rust tests and UI unit tests.
[default]
all: rust ui

# Run workspace tests and doctests.
rust:
    cargo test --locked

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

# Check JMAP against a real server: the Cyrus test server in Docker (needs port 8080 free).
jmap:
    #!/usr/bin/env bash
    set -euo pipefail
    name=colecto-jmap-test
    docker rm -f "$name" > /dev/null 2>&1 || true
    docker run -d --rm --name "$name" -p 127.0.0.1:8080:8080 \
      ghcr.io/cyrusimap/cyrus-docker-test-server@sha256:c5e84adf335bbd945395ec6b8aee1f6cb5675188b2a48d972432678032a01d46 > /dev/null
    trap 'docker rm -f "$name" > /dev/null' EXIT
    for _ in $(seq 90); do curl -sf -u user1:x http://localhost:8080/jmap > /dev/null && break; sleep 1; done
    # No `-p colecto-core`: selecting one package changes dependency features, which would
    # rebuild dozens of crates that `cargo test` (and CI's shared cache) already built.
    COLECTO_JMAP_TEST_URL=http://localhost:8080 cargo test --locked --test jmap_server

# Linux/X11 native launch smoke check (requires xvfb and xwininfo).
native:
    {{j}} build native
    xvfb-run -a bun test ./ui/tests/native.test.mjs

# Test the Linux resource recorder's aggregation, isolation, and comparison rules.
perf:
    python3 -m unittest discover -s scripts/python -p '*_test.py'
