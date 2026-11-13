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
desktop entry, so on Linux install the app for your user:

```sh
just install desktop          # debug build; `just install desktop release` for an optimized one
just cleanup desktop          # remove it again
```

| Installed file | Location |
| --- | --- |
| App binary (UI embedded) | `${XDG_BIN_HOME:-~/.local/bin}/colecto` |
| Menu entry | `${XDG_DATA_HOME:-~/.local/share}/applications/Colecto.desktop` |
| Icons | `${XDG_DATA_HOME:-~/.local/share}/icons/hicolor/*/apps/colecto.{png,svg}` |

The recipe copies the binary instead of pointing the entry at Cargo's target directory,
so later builds from any checkout or `cargo clean` do not change the installed app. To
update, pull and run `just install desktop` again: it builds, then swaps the binary in
with a rename, so a running app keeps working and the next launch starts the new
version. `just cleanup desktop` removes only the files above (and the older
`app.colecto.mail` entry and icons); mail data, settings and logs are kept.

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
not touch your `.colecto` directory. Set `COLECTO_TEST_PORT` to use another port
when checkouts run tests concurrently. The test Host always serves this checkout’s
`ui/dist` (`COLECTO_ASSETS`), and `COLECTO_TEST_HOST_BINARY` runs a copied Host binary
instead of the one in the Cargo target dir, for checkouts that share a target dir. The
test Host never contacts GitHub (`COLECTO_UPDATE_CHECK=off`); tests that need a release
answer mock `/api/v1/releases/latest`.
The shared Playwright fixture skips the startup-screen delay; timing tests opt back
in. Playwright may need system browser libraries.

`ui/tests/html-mail.spec.ts` opens five published company email templates
(`ui/tests/fixtures/html-mail/`, sources and licences in its `README.md`) in Chromium
and WebKit, the engine behind the Linux and macOS apps. After one untimed open that
warms the test browser, each is opened three times from a freshly loaded app. The
median time from the click to the first frame painted after the mail frame's `load`
(text and inline images decoded) must be under 400 ms, and no single open may take a
second. The time is measured inside the page so Playwright's own overhead is
excluded. Each template must also keep remote images blocked and open its button link
outside the app. The fixtures inline their images as `data:` URIs and the test
intercepts every remote request, so nothing goes to the network.

`just perf-check` records CPU and memory samples with build and environment details
under `docs/performance/results/`. See [performance](performance/README.md).

## Icons

Every app icon comes from `crates/colecto-app/icons/icon.svg`. `just icons`
regenerates the desktop set; the release workflow generates the Android and iOS sets
from the same file.

## Branches

| Branch | Holds | Receives |
| --- | --- | --- |
| `develop` | Work merged since the last release | Every feature, fix, docs and CI pull request |
| `main` | The latest release; each release tag points at a commit on it | Only `develop`, through a release pull request |
| `release/vX.Y.Z` | The version bump and release notes for one release | Branched off `develop`, merged back into it |

Branch off `develop` and open pull requests against it (`gh pr create --base develop`).
`main` stays the GitHub default branch, so links to `blob/main/...` show the released
docs and the build caches stay on `main` (see below). Both branches require the
`verify` check.

There is no hotfix path: a fix lands on `develop` and goes out as the next patch
release, together with anything else already merged there.

## Releases

The version lives in one place: `[workspace.package] version` in the root
`Cargo.toml`. Every crate inherits it and the binaries compile it in (reported by
`GET /api/v1/version` and `colecto-daemon --version`). `just bump-version X.Y.Z` sets
it there, in `Cargo.lock` and in `ui/package.json`. A tag that does not match
`Cargo.toml` fails the release before anything builds. Release CI, the Docker image (build argument
`COLECTO_COMMIT`) and the `colecto-git` AUR package also compile in the Git commit
through the `COLECTO_COMMIT` environment variable.

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

### Choosing the version

Agents pick the version from what changed since the last tag. Major versions are the
maintainer's call only.

| Bump | When |
| --- | --- |
| Major (`X`) | Only when the maintainer asks for it in so many words |
| Minor (`Y`) | Anything new for users, extension authors or self-hosters: a feature, setting, endpoint, API or extension capability. Also any breaking change (storage format, API, configuration), since majors are reserved |
| Patch (`Z`) | Nothing new and nothing breaking: bug and security fixes, performance, docs, CI, build, refactors and dependency updates |

If nothing has merged into `develop` since the last tag, there is nothing to release.

### Shipping a release

An agent ships a release only when the maintainer asks for one in the current session
("ship a new release"). The steps:

1. `git fetch --tags origin`. The last release is
   `git tag --list 'v*' --sort=-v:refname | head -1`.
2. Read what changed: `git log --first-parent <last tag>..origin/develop` and the
   pull requests merged into `develop` since then. Open the diff when a title does not
   say what users will notice.
3. Choose the version with the table above, and say why in the pull request.
4. Branch `release/vX.Y.Z` off `origin/develop`, run `just bump-version X.Y.Z`, and
   write `.github/release-notes/vX.Y.Z.md`. Follow the earlier notes: a one-line
   summary, then short sections grouped by what users notice. Write for users, not
   commit by commit, link docs at the tag (`blob/vX.Y.Z/docs/...`), and leave out
   changes nobody outside the repository would notice. Mark roadmap items shipped in
   `docs/roadmap.md`.
5. Open `chore: release vX.Y.Z` against `develop` and merge it once `verify` passes.
6. Open a pull request from `develop` into `main` titled `release: vX.Y.Z` and merge it
   with a merge commit (not squash or rebase), so `main` keeps `develop`'s commits.
7. Tag that merge commit and push the tag, which publishes:
   `git tag -a vX.Y.Z -m "Colecto vX.Y.Z" <merge commit> && git push origin vX.Y.Z`.
8. Follow the Release run and report the release link, or the failing job.

## CI build caches

GitHub lets a run restore caches saved on its own branch or tag, or on `main`. Pull
requests and tags never save; they restore what `main` saved, so an unchanged
dependency tree is never rebuilt from scratch.
`develop` saves nothing either, because the 10 GB budget has no room for a second set:
a dependency change merged into `develop` builds cold in each pull request until the
next release brings it to `main`.

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
  run still compiles Colecto itself. Failed release jobs do not save: this loses
  partial progress, but prevents an immutable final key from retaining an incomplete cache.
- The repository has 10 GB of cache. CI and Release's shared `prune-caches` workflow keeps
  the newest Rust and Bun package cache of each kind per branch and deletes the ones superseded
  by a newer toolchain or lockfile. GitHub also evicts anything unused for
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
  most of the budget. Hosted jobs create a fresh Buildx builder, so the Dockerfile
  registry and target cache mounts only help within that builder; every new job
  pays the cold dependency-build cost. A future `type=gha` layer cache would not
  export those cache-mount contents.

### Measured timings

Measured on GitHub-hosted runners on 2026-11-11. A release's wall time is its
slowest job.

| Run | Before (v0.1.0) | First run, no cache | Cached, thin LTO | Cached, no LTO (current) |
| --- | --- | --- | --- | --- |
| Release, wall time | 34 min | 18 min | 9 min | 7.5 min |
| `cargo build`, Windows | | | 6.5 min | 2.2 min |
| `cargo build`, macOS | | | 4.5 min | 1.6 to 2 min |
| `cargo build`, Linux, Android, iOS | | | 2.5 min | 1.1 to 1.5 min |
| Slowest job | Android, 33 min | Windows, 18 min | Windows, 8.5 min | Android x64, 6 min |
| CI `verify` | 4 min; 12 to 20 min during mirror outages | | | 4 to 4.5 min |
| CI `jmap` | 1.5 min (85 crates rebuilt); 8 to 10 min during mirror outages | | | 1 min |

With caches, most release jobs take 3 to 5 minutes: 1 to 2 minutes compiling
Colecto's own crates, and the rest is setup and bundling (Windows MSI and NSIS
about 50 s, Linux AppImage about 75 s).

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
