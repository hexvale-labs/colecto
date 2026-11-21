# Roadmap

Colecto is in **beta**: one owner can use it as their daily mail client over IMAP/SMTP
or JMAP, without data loss and without opening another client for routine mail.

What beta promises:

- Updates migrate stored data forward (see [storage](storage.md#migrations)). Going back
  to an older version is not supported: an older build refuses a newer database.
- Settings keep only what the owner changed, so improved defaults reach everyone who
  never changed them.

Statuses: **Done**, **In progress**, **Next**, **Later**.

## Mail

| Item | Status |
| --- | --- |
| Reply, reply all, forward; Rust-sanitized HTML reading; attachments both ways | Done |
| IMAP IDLE and JMAP push, with polling fallback | Done |
| IMAP STARTTLS, any SMTP port, a pinned certificate per account (Proton Mail Bridge) | Done; untested against a real Bridge |
| Discover every server folder with its role and counts | Done |
| Read/unread, star, move, archive, and delete | Done |
| Search filters (`from:`, `to:`, `is:`, `after:`, `before:`) run on the server | Done |
| Browse any folder with paging through history | Done |
| Conversation threading and quoted-text folding | Done |
| Inline `cid:` images in HTML mail | Done |
| Incremental refresh of cached folders instead of refetching | Next |
| Larger offline cache (thousands of messages per account) | Next |
| End-to-end encryption (OpenPGP and S/MIME) | Later |

## Storage & Security

| Item | Status |
| --- | --- |
| Single-file `colecto.db` with versioned migrations and pre-migration backups | Done |
| Sparse settings table and relational tables for drafts, labels, API keys, send approvals, and AI caches | Done |
| XChaCha20-Poly1305 credential vault with OS keyring or Argon2id master password | Done |
| Mobile OS `SecretStore` (Android Keystore / iOS Keychain) | Next |
| `colecto-daemon export` of accounts (without secrets), settings, drafts and labels | Later |

## App & Interface

| Item | Status |
| --- | --- |
| Multi-platform Flutter app with per-platform native design systems (`shadcn_ui`, `fluent_ui`, `macos_ui`, Material 3, Cupertino) | Done |
| Adaptive 3-tier navigation, command palette, keyboard shortcuts, zoom, Demo mode, and worst-case stress fixtures | Done |
| In-process `colecto-ffi` (`dart:ffi`) backend with zero loopback ports, bundled in desktop/Android builds | Done |
| Remote HTTP + SSE backend (`RemoteHttpBackend`) for Web and remote daemon | Done |
| Optional native AI: summaries, drafted replies with tones, regex and decision-model auto-labels | Done |
| Scoped API keys, send approvals, and MCP server (`colecto-mcp`) | Done |
| Account setup wizard with autodiscovery (ISPDB, Autoconfig, DNS SRV/MX, JMAP well-known) | Done |
| Rich-text composer | Later |

## Platforms & Hosting

| Item | Status |
| --- | --- |
| Linux, Windows, macOS, Android, and Web release bundles; Docker image `ghcr.io/kogical/colectod`; AUR packages | Done |
| Android: UnifiedPush from a self-hosted daemon (Web Push, no foreground service) | Next |
| Android: optional foreground service for local accounts without a daemon | Next |
| Several users on one daemon | Later |
| Synchronizing UI state between devices | Later |

## Not planned

- Extensions / plugin runtime / Wasm UI sandbox (features are built cleanly as first-party modules)
- POP3
- 32-bit platforms
