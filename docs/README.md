# Colecto documentation

Colecto is an email client for IMAP/SMTP and JMAP. Its mail logic is written in Rust,
its interface in Svelte, and it runs as a desktop/mobile app (Tauri), in the browser,
or as a self-hosted daemon. The whole interface is built from extensions, and
third-party extensions run in a WebAssembly sandbox.

Start with the page that matches what you want to do:

| I want to… | Read |
| --- | --- |
| Install Colecto | [Installing](install.md) |
| Add accounts and understand credential protection | [Accounts and credentials](accounts.md) |
| Set up optional AI summaries, replies and automatic labels | [AI features](ai.md) |
| Run the daemon on a server or in Docker | [Self-hosting](self-hosting.md) |
| Build, run and test from a checkout, branches and releases | [Development](development.md) |
| Understand how the pieces fit together | [Architecture](architecture.md) |
| See what is done and what is next | [Roadmap](roadmap.md) |
| Write an extension | [Make your own extension](extensions/guide.md), then the [extension contract](extensions/README.md) |
| Write an extension in Rust | [Rust extension SDK](extensions/rust-sdk.md) |
| Change how data is stored | [Storage](storage.md) |
| Work on the interface | [Design](DESIGN.md) and [product principles](PRODUCT.md) |
| Measure resource use | [Performance](performance/README.md) |
| Work here as an AI agent | [Agent instructions](../AGENTS.md) |

## Technologies

| Area | Technology |
| --- | --- |
| Mail logic, storage, daemon | Rust (edition 2024), one Cargo workspace |
| Receiving and sending | IMAP (`imap`), SMTP (`lettre`), JMAP (own client over `reqwest`); never POP3 |
| Database | Turso, embedded (one `colecto.db` per data directory) |
| Credentials | OS keyring (`keyring`), XChaCha20-Poly1305 vault, optional Argon2id master password |
| HTTP API and web UI server | `axum` and `tokio` |
| Third-party extensions | WebAssembly run by `wasmi` (no WASI) |
| Automation | MCP server (`rmcp`), scoped API keys with send approval |
| Interface | Svelte 5, TypeScript, Vite; Bun as package manager and runner |
| Desktop and mobile app | Tauri 2 (Linux, Windows and Android supported; macOS and iOS build from source only) |
| Tests | `cargo test`, Playwright, `bun test`; a real JMAP server in Docker |
| Tasks | `just` (recipes in `scripts/just/`) |
| Packaging | GitHub Actions releases, Docker image `ghcr.io/hexvale-labs/colectod`, AUR packages |

## Where things are

| Path | Contents |
| --- | --- |
| `crates/colecto-core/` | Mail transports (IMAP, SMTP, JMAP), MIME decoding, accounts, credential vault, drafts, labels, snapshot cache, Turso storage |
| `crates/colecto-extension-api/` | Version 1 wire types shared by the Host and the UI |
| `crates/colecto-extension/` | Rust SDK for third-party extensions (typed calls, bus messages, mock Host) |
| `crates/colecto-extension-sys/` | The SDK's raw Wasm imports and `run` export; the only SDK crate with `unsafe` code |
| `crates/colecto-package/` | `.colectoext` reading and writing, package limits and manifest rules, shared by the Host and `cargo colecto` |
| `crates/cargo-colecto/` | `cargo colecto`: builds, checks and packages Rust extensions |
| `crates/colecto-host/` | The mail service: HTTP API, auth, extension policy and Wasm sandbox, MCP; binaries `colecto-host`, `colecto-daemon`, `colecto-mcp` |
| `crates/colecto-app/` | Tauri app; runs the mail service in-process and embeds the UI |
| `crates/colecto-native/` | Tauri plugin for the mobile app: Android back gestures, press sounds and haptics (Kotlin, Swift) |
| `ui/src/` | Svelte app shell, settings screens and shared components |
| `ui/src/extensions/<id>/` | Bundled extensions: Svelte code next to its `extension.toml` |
| `ui/src/lib/` | API client, layout, navigation, layer (z-index) registry and helpers |
| `ui/tests/` | Playwright specs (`*.spec.ts`) and Bun tests (`*.test.mjs`) |
| `docs/extensions/examples/` | Example third-party extensions (WAT panel, Oneko overlay built with the Rust SDK) |
| `docs/performance/` | Measurement method, committed baseline, local results (ignored by git) |
| `scripts/just/` | `just` modules: `build`, `check`, `test`, `run`, `install` |
| `scripts/python/`, `scripts/javascript/` | Resource benchmark and AUR publishing helpers |
| `packaging/` | Linux desktop entry, systemd units, Docker Compose, AUR PKGBUILDs |
| `.github/workflows/` | CI (`ci.yml`) and releases (`release.yml`) |
| `.github/release-notes/` | Release notes used when a `vX.Y.Z` tag is published |

## Words we use

- **Core services:** the trusted Rust foundation that owns accounts, credentials,
  mail, drafts and settings.
- **Host:** the small trusted layer that starts the service, loads extensions, checks
  permissions and serves the API.
- **Extension:** an optional package that adds or replaces interface. **Bundled
  extensions** are the first-party Svelte ones; **third-party extensions** are
  `.colectoext` packages running as WebAssembly.
- **Daemon:** the Host running on its own, usually on a server.
- **Owner:** the one person whose accounts a data directory holds.
