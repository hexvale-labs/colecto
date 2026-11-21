# Testing Strategy

Colecto values **quality over quantity** in its test suite. Every test should defend a real
user-visible behavior, security boundary, persistence invariant, or wire/FFI contract—and the
whole local verification suite should finish in seconds, not minutes.

## The Test Trophy

We deliberately favor **integration tests** over heavy unit-test sprawl or brittle E2E suites:

| Layer | Share | What belongs here | What does NOT belong here |
| --- | --- | --- | --- |
| **Static checks** | Foundation | `cargo fmt`, `cargo clippy -D warnings`, `dart format`, `flutter analyze` | — |
| **Unit tests** | Small & sharp | Pure logic with tricky edge cases or security stakes: HTML sanitization (`ammonia`), MIME nesting/size bounds, search query escaping, conversation threading, date/reply formatting, `ServiceEvent.fromWire` decoding | Trivial getters, 5-line `match` mirrors, enum round-trips, or compiler-checked types |
| **Integration tests** | **Primary investment** | Real module boundaries: `colecto-core` + Turso `colecto.db` migrations/vault/restart, `colecto-daemon` HTTP/SSE/MCP routes & auth policy, `colecto-ffi` C-ABI round-trips, Flutter `MailController` + `ColectoApp` full-screen flows, `RemoteHttpBackend` HTTP/SSE wire contracts | Redundant N×M layout matrices that pump the entire app dozens of times without adding new failure modes |
| **E2E tests** | **Strictly critical paths** | Live Flutter ↔ Rust daemon contract (`app/test/daemon_e2e_test.dart`), real IMAP/SMTP (`crates/colecto-core/tests/mail_server.rs`), and real JMAP (`crates/colecto-core/tests/jmap_server.rs`) | Exhaustive UI permutations or slow browser/emulator flows for every minor widget |

## Guidelines for Writing & Reviewing Tests

1. **Test behavior and invariants, not implementation details.**
   If refactoring a private helper breaks a test while behavior stays identical, the test is
   coupled to the wrong layer.
2. **Delete redundant tests when a stronger integration test supersedes them.**
   A subset test that opens a fresh database or pumps the full widget tree to re-check
   something already covered by a broader flow slows everyone down for zero added signal.
3. **Keep E2E tests focused on critical paths.**
   E2E tests are expensive to run and maintain. Limit them to:
   - Auth, session enforcement, and fail-closed startup
   - Reading, organizing, drafting, and sending mail (including attachments)
   - Live event delivery (SSE / FFI) and restart persistence
4. **Design for parallelism and speed.**
   - **Flutter:** Split tests into focused files under `app/test/` so `flutter test --concurrency`
     runs files across multiple workers. Never dump all tests into a single monolithic file.
   - **Layout resilience:** Test every design system (`colecto`, `fluent`, `macos`, `material`,
     `cupertino`) against worst-case data at the narrowest mobile viewport (`360×740`, where
     overflows actually happen), plus representative desktop/tablet viewports—not an unpruned
     cartesian product of every design × every size × every dataset.
   - **Rust:** Prefer `cargo-nextest` (`.config/nextest.toml`) for per-process parallelism and
     slow-test visibility. Consolidate overlapping tests that each spin up a fresh Turso database.

## Running Tests

```sh
just test            # Fast Rust + Flutter suites (parallelized)
just test rust       # Rust workspace tests (uses cargo-nextest when installed, else cargo test)
just test app        # Flutter unit, widget, and integration tests across workers
just test e2e        # Live Flutter RemoteHttpBackend <-> colecto-daemon E2E contract test
just test jmap       # Real JMAP server E2E (Cyrus in Docker, port 8080)
just test mail       # Real IMAP/SMTP E2E (GreenMail in Docker, ports 3993 & 465)
just verify          # Full static checks + fast test suites
```

## CI Execution

`.github/workflows/ci.yml` runs:

- **`rust`**: `cargo fmt`, `cargo clippy --all-targets -D warnings`, `cargo nextest run --workspace --locked --profile ci`, and doctests. Publishes the built `colecto-daemon` test binary for downstream E2E verification.
- **`flutter`**: `dart format`, `flutter analyze`, `flutter test --concurrency=$(nproc)` across all test files in parallel, and the Flutter Web build.
- **`e2e`**: Downloads `colecto-daemon` from the `rust` job (no recompilation) and runs `app/test/daemon_e2e_test.dart` against the live daemon process over HTTP + SSE.
- **`docker`**: Builds the daemon container with bundled Flutter Web assets and smoke-tests fail-closed non-loopback startup and loopback `/api/v1/version`. The `Dockerfile` compiles Rust dependencies in a [cargo-chef](https://github.com/LukeMathWalker/cargo-chef) layer keyed only on manifests and `Cargo.lock`, and CI stores those layers in the GitHub Actions cache (`daemon-docker-<arch>` scope, saved on `develop` and `main`, shared with the release image build), so a routine run recompiles only the workspace crates. Keep `ARG COLECTO_COMMIT` just before the final `cargo build`: a build arg that changes invalidates every `RUN` after it.
