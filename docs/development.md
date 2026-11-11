# Development

How to build, run and check Colecto from a checkout. For what the pieces are and how
they fit together, see [project direction](README.md).

## Prerequisites

Stable Rust (1.89 or newer) with `rustfmt` and `clippy`, [`just`](https://just.systems),
and Bun 1.4.4 or newer. Linux also needs OpenSSL and D-Bus development libraries and
pkg-config (on Debian/Ubuntu: `libssl-dev libdbus-1-dev pkg-config`). The desktop app
additionally needs [Tauri's system dependencies](https://v2.tauri.app/start/prerequisites/).

```sh
just install        # Rust crates and UI packages
just                # list every recipe
```

## Run it

```sh
just run            # browser UI at http://127.0.0.1:4173
just run native     # desktop app (Tauri)
just run daemon     # standalone mail service
```

`just run` binds only to IPv4 loopback; open the printed address exactly rather than
`localhost`. The browser asks for the owner access token: copy it from the private
`.colecto/owner-token` file (or `owner-token` inside `COLECTO_DATA_DIR`). The Host
creates that file with mode `0600`, tightens its data directory to `0700`, never
prints the secret and never puts it in a URL. An existing token file with looser
permissions is rejected.

The desktop app connects by itself: it injects its owner credential only at its own
loopback origin. It keeps its data in the platform's app data directory (for example
`~/.local/share/app.colecto.mail` on Linux) unless `COLECTO_DATA_DIR` is set, and its
logs in the platform's log directory. Wayland desktops take a window's icon from its
desktop entry, so on Linux run `just desktop-install` once.

The Host stores accounts, drafts, labels and Inbox/Sent snapshots in one embedded
Turso database, `colecto.db`, inside the data directory. Drafts and snapshots are not
encrypted at rest. Only one Host may open a data directory at a time. A corrupt or
unsupported store stops startup without overwriting it. See [storage](storage.md).

```sh
COLECTO_PORT=4180 COLECTO_DATA_DIR=/path/to/other-data just run
```

## Logging

Every binary logs through `tracing` to stderr. `COLECTO_LOG` (or `RUST_LOG`) takes a
filter such as `debug` or `warn,colecto_core=trace`; the default is `info` for Colecto
and `warn` for libraries. HTTP requests are logged at `debug` with method and path
only, never query strings. Under systemd, lines carry syslog priorities so
`journalctl -p warning` works; in Docker, `docker logs` shows timestamped lines.

## Check changes

```sh
just verify           # Rust fmt/check/Clippy/tests, Svelte/TypeScript, UI build
just install browsers # install the test browsers once
just test browser     # browser flows and a real Host restart
just test native      # desktop window smoke test (Linux, needs xvfb)
just test jmap        # send/receive/search/attachments against a real JMAP server (Docker)
just perf-check       # verification plus three recorded resource runs
```

The browser tests run a separate Host on port 4179 with temporary sample data and do
not touch your `.colecto` directory. Playwright may need system browser libraries.

`just perf-check` records CPU and memory samples with build and environment details
under `performance-results/`. See [performance](performance.md).

## Icons

Every app icon comes from `crates/colecto-app/icons/icon.svg`. `just icons`
regenerates the desktop set; the release workflow generates the Android and iOS sets
from the same file.

## Releases

Pushing a semver tag such as `v1.2.3` runs `.github/workflows/release.yml`; other
tags, pull requests and pushes to `main` do not. It builds the app
for Linux, Windows, macOS, Android and iOS, names each file
`colecto-v1.2.3-<platform>-<arch>.<ext>`, publishes a GitHub release with
`SHA256SUMS.txt`, and pushes the daemon image to
`ghcr.io/hexvale-labs/colecto-daemon`. To test the pipeline without releasing, start
it by hand from the Actions tab (**Release → Run workflow**): it builds everything and
publishes nothing. Nothing is code-signed; see
[installing](install.md).

`just build release` builds optimized binaries locally.

## Crates

- `colecto-extension-api`: version 1 wire types shared by the Host and extensions.
- `colecto-core`: IMAP/SMTP and JMAP transports, MIME decoding, drafts, accounts,
  credentials and Turso storage.
- `colecto-host`: the mail service. Authenticated HTTP API, static UI, extension
  policy and the Wasm sandbox; also the `colecto-daemon` and `colecto-mcp` binaries.
- `colecto-app`: the Tauri desktop and mobile app. It runs the mail service in-process
  and serves the UI compiled into the binary. It is not a default workspace member,
  because it needs WebView system libraries; build it with `just build native`.
- `ui/src/extensions`: the bundled mailbox, reader and composer views.

## Bun troubleshooting

Run repository tasks with `just`, or run UI scripts from `ui/` (for example
`cd ui && bun run --bun build`). There is no root `package.json`. `bun run` is not a
test runner; `just test browser` runs the restart test with `bun test` and Playwright
separately. Playwright collects only `*.spec.ts`.

The frontend build inherits a low-severity `deepmerge` advisory from
`@sveltejs/vite-plugin-svelte` with no upstream fix. It only merges trusted build
configuration.
