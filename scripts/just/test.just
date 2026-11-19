set working-directory := '../..'
j := just_executable() + " --justfile " + quote(justfile_directory() / "justfile")

# Run the fast suites: Rust tests and UI unit tests.
[default]
all: rust ui

# Run workspace tests and doctests. The Host runs the real AI service part, so build it first.
rust:
    {{j}} build ai-service
    cargo test --locked

# Run UI unit tests without starting a browser or mail service.
ui:
    bun test ./ui/tests/api.test.mjs ./ui/tests/discovery.test.mjs ./ui/tests/reply.test.mjs ./ui/tests/when.test.mjs ./ui/tests/layout.test.mjs ./ui/tests/navigation.test.mjs ./ui/tests/paths.test.mjs

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

# Check IMAP and SMTP against a real server: GreenMail in Docker (needs ports 3993 and 465 free).
mail:
    #!/usr/bin/env bash
    set -euo pipefail
    name=colecto-mail-test
    certs=$(mktemp -d)
    docker rm -f "$name" > /dev/null 2>&1 || true
    trap 'docker rm -f "$name" > /dev/null 2>&1 || true; rm -rf "$certs"' EXIT
    # Colecto pins a certificate by its host name instead of skipping verification, so the
    # server gets a throwaway one for localhost.
    openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj "/CN=localhost" \
      -addext "subjectAltName=DNS:localhost" -keyout "$certs/key.pem" -out "$certs/cert.pem" 2> /dev/null
    openssl pkcs12 -export -in "$certs/cert.pem" -inkey "$certs/key.pem" -name greenmail \
      -passout pass:changeit -out "$certs/keystore.p12"
    chmod 755 "$certs"; chmod 644 "$certs"/*
    docker run -d --rm --name "$name" -p 127.0.0.1:3993:3993 -p 127.0.0.1:465:3465 -v "$certs:/certs:ro" \
      -e GREENMAIL_OPTS="-Dgreenmail.setup.test.all -Dgreenmail.hostname=0.0.0.0 -Dgreenmail.users=user1:password@example.com,user2:password@example.com -Dgreenmail.users.login=email -Dgreenmail.tls.keystore.file=/certs/keystore.p12 -Dgreenmail.tls.keystore.password=changeit" \
      greenmail/standalone@sha256:38f705c272ad8e4dc5ee53add4276e654202802b2bbdf4cb49698dda5102149a > /dev/null
    for _ in $(seq 60); do
      echo | timeout 2 openssl s_client -connect 127.0.0.1:465 > /dev/null 2>&1 && \
        echo | timeout 2 openssl s_client -connect 127.0.0.1:3993 > /dev/null 2>&1 && break
      sleep 1
    done
    COLECTO_MAIL_TEST_CERT="$certs/cert.pem" cargo test --locked --test mail_server

# Linux/X11 native launch smoke check (requires xvfb and xwininfo).
native:
    {{j}} build native
    xvfb-run -a bun test ./ui/tests/native.test.mjs

# Test the Linux resource recorder's aggregation, isolation, and comparison rules.
perf:
    python3 -m unittest discover -s scripts/python -p '*_test.py'
