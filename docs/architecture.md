# Architecture

Colecto pairs a shared **Rust mail engine** with a **Flutter multi-platform UI**, with no extension runtime.

## High-Level Topology

```
┌────────────────────────────────────────────────────────────────────┐
│  Flutter App (app/)                                                │
│  Shared state/controllers + Colecto kit (one design per platform)     │
│  - Linux & Web (Wasm): shadcn_ui   - Windows: fluent_ui            │
│  - macOS: macos_ui   - Android: Material 3   - iOS: Cupertino      │
├─────────────────────────────────┬──────────────────────────────────┤
│  LocalFfiBackend                │  RemoteHttpBackend               │
│  (flutter_rust_bridge, in-proc) │  (HTTP + SSE)                    │
└────────────────┬────────────────┴────────────────┬─────────────────┘
                 │                                 │
┌────────────────▼──────────────┐ ┌────────────────▼─────────────────┐
│  colecto-ffi (Rust)           │ │  colecto-daemon (HTTP/SSE/MCP)  │
│  Thin C-ABI FFI facade over   │ │  Auth, API keys, MCP stdio       │
│  colecto-service              │ │  (`colecto-mcp`), static Web UI  │
└────────────────┬──────────────┘ └────────────────┬─────────────────┘
                 │                                 │
┌────────────────▼─────────────────────────────────▼─────────────────┐
│  colecto-service (Rust, transport-agnostic)                       │
│  `MailService`, sync/IDLE/push, autodiscovery, native AI           │
│  summaries/replies/auto-labeling, preferences, approvals, API keys │
└────────────────────────────────┬───────────────────────────────────┘
                                 │
┌────────────────────────────────▼───────────────────────────────────┐
│  colecto-core (Rust)                                              │
│  IMAP/SMTP, JMAP, MIME, `ammonia` HTML sanitizer, threading,       │
│  Turso `colecto.db`, XChaCha20-Poly1305 vault, Argon2id,          │
│  accounts, drafts, labels, search, offline cache                   │
└────────────────────────────────────────────────────────────────────┘
```

## Workspace Layout

| Path | Role |
| --- | --- |
| `crates/colecto-core/` | Pure Rust mail engine ported from `colecto-core` (inlined domain types, IMAP/SMTP, JMAP, MIME, `ammonia` HTML sanitization in `html.rs`, conversation threading in `threading.rs`, Turso `colecto.db` with forward-only `PRAGMA user_version` migrations & pre-migration backups, XChaCha20-Poly1305 vault) |
| `crates/colecto-service/` | HTTP-free, reusable `MailService` orchestration (sync/IDLE/push, autodiscovery, native AI summaries/replies/auto-labeling backed by `ai_summaries` & `ai_autolabel_seen`, sparse `settings` preferences, scoped API keys `colecto_*`, send approvals, notifications, release checks) |
| `crates/colecto-daemon/` | Standalone HTTP/SSE/MCP daemon binary (`colecto-daemon`) and stdio MCP broker (`colecto-mcp`), `x-colecto-account` header, `COLECTO_*` env vars, zero sidecar token/config files |
| `crates/colecto-ffi/` | Thin C-ABI FFI facade (`libcolecto_ffi`) exposing `MailService` and `ServiceEvent` polling to Dart (`dart:ffi`), bundled automatically by Flutter desktop and Android builds |
| `app/` | Flutter application (Android, iOS, Linux, Windows, macOS, Web); screens are built from the Colecto kit (`lib/ui/core/kit/`), which maps onto each platform's design library; single `MailBackend` interface (`LocalFfiBackend`, `RemoteHttpBackend`, and `InMemoryMailBackend` demo/test backend) |
| `docs/` | Architecture, UI specification, decisions, storage, accounts, self-hosting, and development guides |

## Porting Status from `kogical/colecto`

Reference checkout: `github.com/kogical/colecto` (`origin/develop`).

### Ported and Cleaned Up
- **`crates/colecto-core/`** (from `colecto-core`):
  - `accounts.rs`, `cache.rs`, `content.rs`, `credentials.rs`, `database.rs`, `jmap.rs`, `labels.rs`, `lib.rs`, `mime.rs`, `remote.rs`, `search.rs`, plus `types.rs` (inlined domain structs), `html.rs` (`ammonia` sanitizer with remote-image blocking and `cid:` resolution), and `threading.rs`.
  - Consolidated `migrations/0001_initial.sql` for `colecto.db` including `ai_summaries` and `ai_autolabel_seen`.
- **`crates/colecto-service/`** (from non-extension `colecto-host` + native AI service):
  - `lib.rs` (`MailService`), `error.rs`, `events.rs`, `sync.rs`, `discovery.rs`, `ai.rs` (combining host AI + native auto-label/reclassify without Wasm), `api_keys.rs`, `approvals.rs`, `notifications.rs`, `preferences.rs`, `settings.rs`, `secrets.rs`, `store.rs`, `logging.rs`, `releases.rs`.
- **`crates/colecto-daemon/`** (from `colecto-host` HTTP/MCP/runtime):
  - `lib.rs` (`Daemon`, `AccessPolicy`, `Assets`, `/api/v1/*` routes), `mcp.rs` (20 MCP tools over `MailService`), `runtime.rs`, `bin/colecto-daemon.rs`, `bin/colecto-mcp.rs`.

### Left Behind (Not Ported)
- Crates: `colecto-extension-api`, `colecto-extension`, `colecto-extension-sys`, `colecto-package`, `cargo-colecto`, `colecto-app`, `colecto-native`, Wasm `colecto-ai-service`.
- Host modules: `bus.rs`, `extensions.rs`, `sandbox.rs`, `services.rs`.
- Dependencies: `wasmi`, `wat`, `zip`, `tauri`, `tauri-build`.
- Sidecar files: `owner-token`, `daemon.json`, `colecto-settings.json`.
- The entire `ui/` Svelte codebase.

### Mobile Cross-Compilation Notes
- **TLS:** `colecto-core` currently uses `native-tls` (`imap`, `lettre`, `reqwest`). Evaluate switching to `rustls` if OpenSSL cross-compilation causes friction on Android/iOS targets.
- **Keychain / Keystore:** OS secret storage is behind `colecto_core::SecretStore` so desktop uses OS keyring while Android/iOS can plug into Android Keystore / iOS Keychain cleanly.

## Backend Modes

1. **Standalone (Desktop & Mobile):**
   - Flutter invokes `colecto-ffi` in-process via `flutter_rust_bridge` directly against `colecto-service::MailService`.
   - Zero loopback HTTP ports, zero custom-scheme workarounds.
2. **Web (Flutter Wasm) & Remote Daemon:**
   - Browsers cannot open raw TCP sockets for IMAP/SMTP, so Web is 100% pure Dart talking to `colecto-daemon` over HTTP + SSE (no Rust-to-Wasm compilation of `colecto-core` required).
   - Desktop and mobile apps can also switch to `RemoteHttpBackend` to connect to a self-hosted `colecto-daemon`, with Android receiving wakeups via UnifiedPush.

## Running `colecto-daemon`

### Quick Start (`just`)

- **Web UI + daemon (local test drive):** `just run web` builds the Rust workspace and Flutter Web bundle, then launches `colecto-daemon` with `COLECTO_ASSETS=app/build/web` on `http://127.0.0.1:4173`. Open that URL and paste the printed ephemeral bearer token (remembered in `sessionStorage` for the tab), or click **Demo Mode**.
- **Native Flutter client against a running daemon:** `just run app linux http://127.0.0.1:4173 <token>` (or `just run demo` for built-in demo data).
- **Compile-time flags:** `COLECTO_BACKEND` (`demo` | `remote` | `ffi`), `COLECTO_DAEMON_URL`, and `COLECTO_TOKEN` can be passed via `--dart-define`.
- **Full verification:** `just verify` runs Rust formatting, `cargo check`, Clippy (`-D warnings`), `cargo test`, Dart formatting, `flutter analyze`, and `flutter test`.

### Modes

- **Local loopback (default):** `cargo run -p colecto-daemon` binds `127.0.0.1:4173` and accepts both `127.0.0.1:<port>` and `localhost:<port>` as the `Host` / `Origin`. When `COLECTO_TOKEN_FILE` is omitted, a random ephemeral bearer token is generated and printed once to stdout (`Access token (ephemeral): ...`); no sidecar token file is written.
- **Local Unix socket:** Set `COLECTO_SOCKET=/path/to/private/mail.sock` (parent directory must be `chmod 700`; socket is created `chmod 600`; requests use `Host: localhost`). Mutually exclusive with `COLECTO_PUBLIC_ORIGIN`.
- **Remote / non-loopback:** Binding any non-loopback IP (`COLECTO_BIND`) requires **both** `COLECTO_PUBLIC_ORIGIN` (an `https://` origin with no path, query, or credentials) and `COLECTO_TOKEN_FILE` (owner-readable `0600` file containing a 64–512 character ASCII `[A-Za-z0-9_-]` token).
- **Notification relay:** Set `COLECTO_RELAY_ORIGIN` and `COLECTO_TOKEN_FILE` to relay SSE notifications from a remote daemon to local desktop notifications without syncing mail locally.
- **Authentication & `Origin` handling:** Auth is strictly `Authorization: Bearer <token>` (no cookies). Browser requests that include an `Origin` header must match the allowed origin; native HTTP clients such as `RemoteHttpBackend` may omit `Origin` when presenting a valid bearer token.

### Environment Variables

| Variable | Binary | Default / Requirement |
| --- | --- | --- |
| `COLECTO_DATA_DIR` | `colecto-daemon` | `.colecto` (stores `colecto.db`) |
| `COLECTO_BIND` | `colecto-daemon` | `127.0.0.1`; non-loopback requires `COLECTO_PUBLIC_ORIGIN` + `COLECTO_TOKEN_FILE` |
| `COLECTO_PORT` | `colecto-daemon` | `4173` |
| `COLECTO_SOCKET` | `colecto-daemon` | Unset; Unix socket path inside a `0700` directory |
| `COLECTO_PUBLIC_ORIGIN` | `colecto-daemon` | Unset; `https://host[:port]` for remote deployments |
| `COLECTO_TOKEN_FILE` | `colecto-daemon` | Unset; `0600` file with 64–512 char owner token (required for remote & relay modes) |
| `COLECTO_ASSETS` | `colecto-daemon` | Unset; optional directory of static web UI assets |
| `COLECTO_SYNC_SECONDS` | `colecto-daemon` | `60` (minimum `10`) |
| `COLECTO_RELAY_ORIGIN` | `colecto-daemon` | Unset; runs in desktop notification relay mode |
| `COLECTO_NOTIFICATIONS` | `colecto-daemon` | Enabled on Linux D-Bus sessions in Unix socket mode; set `off` to disable |
| `COLECTO_UPDATE_CHECK` | `colecto-daemon` | Enabled; set `off` to disable background release checks |
| `COLECTO_LOG` | `colecto-daemon`, `colecto-mcp` | `RUST_LOG` fallback; `tracing` `EnvFilter` directive |
| `COLECTO_MCP_KEY_FILE` / `COLECTO_MCP_KEY` | `colecto-mcp` | Required (`colecto_*` scoped API key for the stdio MCP bridge) |

### Docker

The repository ships a multi-stage [`Dockerfile`](../Dockerfile) that builds a minimal non-root (`10001:10001`) `debian:bookworm-slim` image containing `colecto-daemon`, `colecto-mcp`, `ca-certificates`, `libssl3`, `libdbus-1-3`, and (when built with `--build-context web-assets=app/build/web`, as in CI and `just build docker`) the Flutter Web UI under `/srv/colecto/web`.

Defaults in the image: `COLECTO_BIND=0.0.0.0`, `COLECTO_PORT=4173`, `COLECTO_DATA_DIR=/data`, `COLECTO_ASSETS=/srv/colecto/web`, `COLECTO_NOTIFICATIONS=off`.

What else is needed to run the daemon in Docker:
- **Persistent `/data` volume** owned by UID/GID `10001:10001` (mode `0700`) for `colecto.db` and pre-migration backups.
- **HTTPS reverse proxy** (Caddy, Traefik, nginx, etc.) terminating TLS for `COLECTO_PUBLIC_ORIGIN` (`https://mail.example.com`) and forwarding `Host` and SSE streams to port `4173`.
- **Owner token file (`COLECTO_TOKEN_FILE`)** mounted as a regular file (not a symlink), owned by UID `10001`, mode `0600`, containing a 64–512 character `[A-Za-z0-9_-]` token.
- **Master password protection:** Containers have no desktop D-Bus/keyring service, so enable master password protection in **Settings → Security** before adding mail accounts (credentials are sealed in `colecto.db` with Argon2id + XChaCha20-Poly1305).

## HTML Email Rendering & Memory Budget

- **Plain-text & simple messages:** Render natively with Flutter text widgets (zero WebView RAM).
- **Untrusted HTML messages:**
  1. Rust sanitizes the HTML (strips `<script>`, forms, unsafe handlers; blocks remote images by default unless allowed per-sender or in Privacy settings; resolves `cid:` inline images).
  2. Flutter renders the sanitized HTML in a **single lazy/pooled sandboxed WebView** that mounts only when an HTML message is open and releases/reuses when closed.
  3. On **Web (Wasm)**, render in a real `<iframe sandbox>` via Flutter's `HtmlElementView`.
  4. External links (`http`, `https`, `mailto`) are intercepted and opened via the OS browser / mail handler, never inside the embedded WebView.
