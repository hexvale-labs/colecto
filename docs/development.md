# Development

How to build, run and check Colecto from a checkout. For how the pieces fit together, see
[architecture](architecture.md); for where files live, see the
[documentation index](README.md#where-things-are).

## Prerequisites

- Stable Rust (1.89 or newer) with `rustfmt` and `clippy`
- Flutter (stable channel) with Dart
- [`just`](https://just.systems)
- On Linux: OpenSSL and D-Bus development libraries, pkg-config, and Flutter Linux desktop
  build dependencies (on Debian/Ubuntu: `libssl-dev libdbus-1-dev pkg-config clang cmake ninja-build libgtk-3-dev liblzma-dev`)

```sh
just install        # Fetch Rust crates and Flutter packages
just                # List every recipe
```

## Run it

```sh
just run web        # Build Rust + Flutter Web and serve at http://127.0.0.1:4173
just run demo       # Run the native Flutter app (Linux) with built-in demo data
just run daemon     # Run colecto-daemon standalone on 127.0.0.1:4173
just run app linux http://127.0.0.1:4173 <token>  # Connect native app to a running daemon
just run mcp        # Run the stdio MCP bridge
```

To preview another platform's design system on any machine:

```sh
just run demo linux colecto     # shadcn_ui (Linux/Web default)
just run demo linux fluent      # Windows Fluent
just run demo linux macos       # macOS UI
just run demo linux material    # Android Material 3
just run demo linux cupertino   # iOS Cupertino
```

Pass `data=worst` (`just run demo linux native worst`) to stress-test the UI with
worst-case demo data.

### Local in-process FFI (`colecto-ffi`)

Standalone desktop and mobile builds load `libcolecto_ffi` directly via `dart:ffi` with
no loopback TCP port:

```sh
just build ffi      # Builds target/release/libcolecto_ffi.{so,dylib,dll}
just build linux    # Builds libcolecto_ffi + Flutter Linux bundle (auto-bundles libcolecto_ffi.so)
```

The CMake (`app/linux/CMakeLists.txt`, `app/windows/CMakeLists.txt`), macOS Xcode
(`app/macos/scripts/bundle_colecto_ffi.sh`), and Android Gradle
(`app/android/app/build.gradle.kts`) build scripts automatically pick up `libcolecto_ffi`
from `target/` (or `$COLECTO_FFI_LIB`) when present.

## Check changes

See [Testing](testing.md) for the test strategy (quality over quantity, integration-first,
critical-path E2E, and multi-worker execution).

```sh
just verify         # Rust fmt/check/clippy/test + Dart format/analyze/test
just fmt            # Format Rust and Dart source files
just check          # Static checks only (fmt, cargo check, clippy, flutter analyze)
just test           # Fast Rust + Flutter tests (parallelized across workers)
just test e2e       # Live Flutter RemoteHttpBackend <-> colecto-daemon E2E test
just test jmap      # Real JMAP server E2E (Cyrus in Docker)
just test mail      # Real IMAP/SMTP E2E (GreenMail in Docker)
```

## Logging

Every Rust binary logs through `tracing` to stderr. `COLECTO_LOG` (or `RUST_LOG`) sets
the filter (such as `debug` or `warn,colecto_core=trace`); the default is `info` for
Colecto and `warn` for libraries.

## Branches

| Branch | Holds | Receives |
| --- | --- | --- |
| `develop` | Work merged since the last release | Every feature, fix, docs and CI pull request |
| `main` | The latest release; each release tag points at a commit on it | Only `develop`, through a release pull request |
| `release/vX.Y.Z` | The version bump and release notes for one release | Branched off `develop`, merged back into it |

Branch off `develop` and open pull requests against it. `main` stays the GitHub default
branch so links to `blob/main/...` show the released docs.

## Releases

The version lives in `[workspace.package] version` in the root `Cargo.toml` and `version:`
in `app/pubspec.yaml`. `just bump-version X.Y.Z` updates `Cargo.toml`, `Cargo.lock`, and
`app/pubspec.yaml` together, setting the build number (the Android `versionCode`) to
`X*1000000 + Y*1000 + Z` so each release installs over the previous one. A tag that does
not match fails the release workflow before anything builds.

Pushing a semver tag such as `v0.10.0` runs `.github/workflows/release.yml`:

- Builds Flutter + `colecto-ffi` + `colecto-daemon` + `colecto-mcp` bundles for Linux
  (`x64`, `arm64`), Windows (`x64`), macOS (`arm64`), Android (`arm64`, `x64` APKs), and
  Web (`colecto-vX.Y.Z-web.tar.gz`)
- Publishes a GitHub release with `SHA256SUMS.txt` using `.github/release-notes/<tag>.md`
  when present
- Pushes the multi-arch daemon image (`amd64`, `arm64`) with the Flutter Web UI bundled to
  `ghcr.io/kogical/colectod` (`vX.Y.Z`, `vX.Y`, `vX`, `latest`)
- Updates the `colecto`, `colecto-bin`, and `colecto-git` AUR packages when
  `AUR_SSH_PRIVATE_KEY` is configured

### Choosing the version

| Bump | When |
| --- | --- |
| Major (`X`) | Only when the maintainer asks for it explicitly |
| Minor (`Y`) | Anything new for users or self-hosters, or any breaking change |
| Patch (`Z`) | Bug and security fixes, performance, docs, CI, build, refactors, dependency updates |

### Shipping a release

Ship a release only when the maintainer asks for one in the current session:

1. `git fetch --tags origin` and check `git tag --list 'v*' --sort=-v:refname | head -1`.
2. Review `git log --first-parent <last tag>..origin/develop` and pick the version.
3. Branch `release/vX.Y.Z` off `origin/develop`, run `just bump-version X.Y.Z`, and write
   `.github/release-notes/vX.Y.Z.md` (moving anything in `.github/release-notes/pending.md`).
4. Open `chore: release vX.Y.Z` against `develop` and merge once checks pass.
5. Open a PR from `develop` into `main` titled `release: vX.Y.Z` and merge with a merge
   commit.
6. Tag that merge commit (`git tag -a vX.Y.Z -m "Colecto vX.Y.Z" <sha> && git push origin vX.Y.Z`).
