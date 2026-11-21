set working-directory := '..'
j := just_executable() + " --justfile " + quote(justfile_directory() / "justfile")

# Run the fast parallel test suites (Rust + Flutter).
[default]
all: rust app

# Run Rust workspace tests (uses cargo-nextest when installed, else cargo test) and doctests.
rust:
    #!/usr/bin/env bash
    set -euo pipefail
    if command -v cargo-nextest >/dev/null 2>&1; then
        cargo nextest run --workspace --locked
        cargo test --workspace --doc --locked
    else
        cargo test --workspace --locked
    fi

# Run Flutter unit, widget, and integration tests in parallel across CPU workers.
app:
    #!/usr/bin/env bash
    set -euo pipefail
    workers=$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 2)
    cd app && flutter test --concurrency="$workers"

# Run the critical-path Flutter RemoteHttpBackend <-> live colecto-daemon E2E contract test.
e2e:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build -p colecto-daemon --bin colecto-daemon --locked
    target_dir=$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
    cd app && COLECTO_DAEMON_BIN="$target_dir/debug/colecto-daemon" flutter test test/daemon_e2e_test.dart --reporter=expanded

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
    COLECTO_JMAP_TEST_URL=http://localhost:8080 cargo test --locked --test jmap_server

# Check IMAP and SMTP against a real server: GreenMail in Docker (needs ports 3993 and 465 free).
mail:
    #!/usr/bin/env bash
    set -euo pipefail
    name=colecto-mail-test
    certs=$(mktemp -d)
    docker rm -f "$name" > /dev/null 2>&1 || true
    trap 'docker rm -f "$name" > /dev/null 2>&1 || true; rm -rf "$certs"' EXIT
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
