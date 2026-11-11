# Development

How to build, run and check Colecto from a checkout. For what the pieces are and how
they fit together, see [architecture](architecture.md); for where files live, see the
[documentation index](README.md#where-things-are).

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
under `docs/performance/results/`. See [performance](performance/README.md).

## Icons

Every app icon comes from `crates/colecto-app/icons/icon.svg`. `just icons`
regenerates the desktop set; the release workflow generates the Android and iOS sets
from the same file.

## Releases

Pushing a semver tag such as `v1.2.3` runs `.github/workflows/release.yml` and
publishes. Pushes to `main` that change `Cargo.lock`, a `Cargo.toml`, the workflow or
`.github/actions/` also run it, as does a schedule on Mondays and Thursdays; those runs
only build, to refresh the build caches (see below). It builds the app
for Linux, Windows, Android, and unsigned macOS and iOS, names each file
`colecto-v1.2.3-<platform>-<arch>.<ext>`, publishes a GitHub release with
`SHA256SUMS.txt`, and pushes the daemon image to
`ghcr.io/hexvale-labs/colectod` (tags `vX.Y.Z`, `vX.Y`, `vX`, `latest`). To test the pipeline without releasing, start
it by hand from the Actions tab (**Release → Run workflow**): it builds everything and
publishes nothing. Tick **artifacts** to download the installers from the run; only tag
runs and runs with that box ticked upload them, so the other runs cannot be failed
by an outage of GitHub's artifact service. If a tag run fails on an upload, use
**Re-run failed jobs**: the build caches make the rerun quick. Nothing is code-signed; see
[installing](install.md). Release notes come from `.github/release-notes/<tag>.md`
when that file exists. When the `AUR_SSH_PRIVATE_KEY` secret is set, the release also
updates the `colecto`, `colecto-bin` and `colecto-git` AUR packages (see
`packaging/aur/README.md`).

`just build release` builds optimized binaries locally.

## CI build caches

GitHub lets a run restore caches saved on its own branch or tag, or on `main`. Pull
requests and tags never save; they restore what `main` saved, so an unchanged
dependency tree is never rebuilt from scratch.

| Cache | Saved by | Key |
| --- | --- | --- |
| Rust dependencies for CI (`verify`, also restored by `jmap`) | `ci.yml` on `main` | toolchain + `Cargo.lock` + root `Cargo.toml` |
| Rust dependencies per release target (`release-<triple>`) | `release.yml` on `main` | toolchain + `Cargo.lock` + root `Cargo.toml` |
| Gradle distribution and downloads | `release.yml` on `main`, `android-arm64` job | Tauri CLI version |
| Bun packages, Playwright browsers | `ci.yml` on `main` | `ui/bun.lock`, Playwright version |

- The root `Cargo.toml` hash is part of each Rust cache's `shared-key`, because
  rust-cache itself ignores `[profile.*]` changes. Without it, a profile change keeps
  restoring caches built for the old profile, which Cargo cannot use, and never saves
  new ones.
- Only dependencies are cached (Swatinem/rust-cache drops workspace crates), so every
  run still compiles Colecto itself.
- The repository has 10 GB of cache. CI's `prune-caches` job deletes Rust caches
  superseded by a newer toolchain or lockfile. GitHub also evicts anything unused for
  7 days, which is why `release.yml` runs twice a week.
- `just test jmap` runs workspace-wide (`cargo test --test jmap_server`), not with
  `-p colecto-core`: selecting one package changes dependency features and would
  rebuild dozens of crates the shared cache already holds.
- `.github/actions/apt-install` times a small download from each Ubuntu mirror and
  lists the working ones fastest first, because the runners' default mirror sometimes
  stops resolving or slows to a crawl. It skips `apt-get update` unless the image's
  package lists are stale. CI installs Playwright's libraries through it instead of
  `playwright install --with-deps`.
- The release profile has no LTO. Thin LTO re-optimized every cached dependency at
  each link: rebuilding the app after a change took 700 s with it and 255 s without
  (446 s of link time down to 4.5 s, measured locally with one job), and the binary
  was larger.
- The daemon image has no layer cache. Its job (about 5 minutes) is never the slowest
  in a release, and the Rust caches (750 to 850 MB per release target) already fill
  most of the budget.

### Measured timings

Measured on GitHub-hosted runners on 2026-11-11. A release's wall time is its
slowest job.

| Run | Before (v0.1.0) | First run, no cache | Cached |
| --- | --- | --- | --- |
| Release, wall time | 34 min | 18 min | 9 min |
| Android | 33 min (4 ABIs on one runner) | 12 min per ABI | 4 to 6 min |
| Linux | 8 to 11 min | 8 to 9 min | 5 min |
| Windows | 12 to 15 min | 12 to 18 min | 6.5 to 8.5 min |
| macOS and iOS | 8 to 16 min | 8 to 12 min | 5 min |
| CI `verify` | 4 min; 12 to 20 min during mirror outages | | 4 to 4.5 min |
| CI `jmap` | 1.5 min (85 crates rebuilt); 8 to 10 min during mirror outages | | 1 min |

Android is faster without a cache too: it lost the 32-bit ABIs and builds each ABI on
its own runner.

## Stress fixtures

`cd ui && bun run vite --host 127.0.0.1` serves a development-only `/__stress` route
with adversarial sample data (long names, right-to-left and CJK text, emoji, empty
fields, many rows). The picker state lives in the URL. Production builds drop the
route and its fixtures.

## Bun troubleshooting

Run repository tasks with `just`, or run UI scripts from `ui/` (for example
`cd ui && bun run --bun build`). There is no root `package.json`. `bun run` is not a
test runner; `just test browser` runs the restart test with `bun test` and Playwright
separately. Playwright collects only `*.spec.ts`.

The frontend build inherits a low-severity `deepmerge` advisory from
`@sveltejs/vite-plugin-svelte` with no upstream fix. It only merges trusted build
configuration.
