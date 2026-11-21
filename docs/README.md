# Colecto documentation

Colecto is a multi-platform email client for IMAP/SMTP and JMAP. Its mail engine is
written in Rust, its interface in Flutter with native per-platform design systems, and it
runs standalone (in-process via `colecto-ffi`), in the browser (Flutter Web), or against a
self-hosted `colecto-daemon`. Colecto has no extension runtime; features are built as
clean, first-party modules.

Start with the page that matches what you want to do:

| I want to… | Read |
| --- | --- |
| Install Colecto | [Installing](install.md) |
| Add accounts and understand credential protection | [Accounts and credentials](accounts.md) |
| Set up optional AI summaries, replies and automatic labels | [AI features](ai.md) |
| Run the daemon on a server or in Docker | [Self-hosting](self-hosting.md) |
| Build, run and test from a checkout, branches and releases | [Development](development.md) |
| Understand how the pieces fit together | [Architecture](architecture.md) |
| Understand the UI, navigation and native design systems | [UI design](ui-design.md) |
| Read the stack and architecture decisions | [Decisions](decisions.md) |
| Change how data is stored | [Storage](storage.md) |
| See what is done and what is next | [Roadmap](roadmap.md) |
| Work here as an AI agent | [Agent instructions](../AGENTS.md) |

## Technologies

| Area | Technology |
| --- | --- |
| Mail logic, storage, service, daemon | Rust (edition 2024), one Cargo workspace |
| Receiving and sending | IMAP (`imap`), SMTP (`lettre`), JMAP (own client over `reqwest`); never POP3 |
| HTML sanitization | `ammonia` in Rust (strips scripts/forms, blocks remote images by default) |
| Database | Turso, embedded (one `colecto.db` per data directory) |
| Credentials | OS keyring (`keyring`), XChaCha20-Poly1305 vault, optional Argon2id master password |
| HTTP API and web UI server | `axum` and `tokio` |
| In-process desktop/mobile bridge | `colecto-ffi` (`cdylib`/`staticlib`, called via `dart:ffi`, zero loopback ports) |
| Automation | MCP server (`rmcp`), scoped API keys with send approval |
| Interface | Flutter (Dart), Colecto kit mapping onto `shadcn_ui`, `fluent_ui`, `macos_ui`, Material 3, and Cupertino |
| Tasks | `just` (recipes in `just/`) |
| Packaging | GitHub Actions releases, Docker image `ghcr.io/kogical/colectod`, AUR packages |

## Where things are

| Path | Contents |
| --- | --- |
| `crates/colecto-core/` | Mail transports (IMAP, SMTP, JMAP), MIME decoding, `ammonia` HTML sanitization, threading, accounts, credential vault, drafts, labels, snapshot cache, Turso storage |
| `crates/colecto-service/` | Transport-agnostic `MailService`: sync/IDLE/push, autodiscovery, native AI summaries/replies/auto-labeling, sparse settings, scoped API keys, send approvals, notifications, release checks |
| `crates/colecto-daemon/` | Standalone HTTP/SSE/MCP daemon (`colecto-daemon`) and stdio MCP broker (`colecto-mcp`) |
| `crates/colecto-ffi/` | Thin C-ABI FFI facade (`libcolecto_ffi`) exposing `MailService` and events to Flutter in-process |
| `app/` | Flutter multi-platform UI (Linux, Windows, macOS, Android, iOS, Web) |
| `just/` | `just` modules: `install`, `build`, `check`, `test`, `run` |
| `packaging/` | Linux desktop entry, systemd unit, Docker Compose, AUR PKGBUILDs |
| `.github/workflows/` | CI (`ci.yml`), cache pruning (`prune-caches.yml`), and releases (`release.yml`) |
| `.github/release-notes/` | Release notes used when a `vX.Y.Z` tag is published; `pending.md` collects notes for the next release |
