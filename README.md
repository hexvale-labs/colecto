<p align="center">
  <img src="docs/branding/logo-square.svg" alt="" width="96" height="96">
</p>

<h1 align="center">Colecto</h1>

<p align="center">
  A fast, keyboard-friendly email client for IMAP/SMTP and JMAP.<br>
  Rust core, Svelte UI built from swappable extensions, and a daemon you can host yourself.
</p>

<p align="center">
  <a href="https://github.com/hexvale-labs/colecto/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/hexvale-labs/colecto?sort=semver&display_name=tag&label=release&color=a2d6b1"></a>
  <a href="https://github.com/hexvale-labs/colecto/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/hexvale-labs/colecto/ci.yml?branch=main&label=CI"></a>
  <a href="https://github.com/hexvale-labs/colecto/pkgs/container/colecto-daemon"><img alt="Docker image" src="https://img.shields.io/badge/docker-ghcr.io-2496ed?logo=docker&logoColor=white"></a>
  <img alt="Platforms" src="https://img.shields.io/badge/platforms-Linux%20%7C%20Windows%20%7C%20macOS%20%7C%20Android%20%7C%20iOS%20%7C%20web-555">
  <img alt="Built with Rust and Tauri" src="https://img.shields.io/badge/built%20with-Rust%20%2B%20Tauri-b7410e?logo=rust">
</p>

<p align="center">
  <a href="https://github.com/hexvale-labs/colecto/releases/latest"><b>Download</b></a> ·
  <a href="docs/docker.md"><b>Self-host with Docker</b></a> ·
  <a href="docs/development.md"><b>Build from source</b></a>
</p>

> [!NOTE]
> Colecto is pre-alpha. It works with real mail accounts, but storage formats may still
> change between versions. See the [roadmap](docs/alpha.md).

## Features

- **IMAP/SMTP and JMAP**, side by side, with as many accounts as you like
- **New mail in seconds** through IMAP IDLE and JMAP push, with polling as a fallback
- **Read safely:** HTML mail renders in a script-free sandbox, and remote images wait
  for your OK (or load automatically, if you choose in Settings → Privacy)
- **Reply, reply all and forward**, with attachments in both directions
- **Search** Inbox and Sent through your provider
- **Local drafts and labels**, kept in a single embedded database
- **Your layout:** panes split, tab and dock like an editor, and the layout follows
  you between devices
- **Keyboard first:** command palette (<kbd>Ctrl</kbd>+<kbd>K</kbd>), <kbd>C</kbd> to
  compose, keyboard-resizable panes
- **Extensions:** the whole UI is built from bundled extensions, and third-party
  panels run in an isolated WebAssembly sandbox
- **Self-hostable daemon** that keeps receiving mail while every app is closed, with
  [Docker](docs/docker.md), systemd or any other supervisor
- **Automation with guard rails:** scoped API keys and an MCP server, with your
  approval required before anything is sent
- **Encrypted credentials**, protected by your system keychain or a master password
- **Notifications** in the app and on the desktop, positioned where you want them
- **Your accent colour**, on a calm dark interface
- **Runs everywhere:** Linux, Windows, macOS, Android, iOS and the browser

Have an idea? [Open an issue](https://github.com/hexvale-labs/colecto/issues/new) and
tell us about it.

### Planned

- All server folders, with paging and incremental sync
- Read/unread, star, archive, delete and move
- Conversation threading
- Inline (`cid:`) images in HTML mail
- Encryption of stored mail
- Native push notifications on Android and iOS
- Hosting several users on one daemon

### Not planned

- POP3

## Get started

1. [Download Colecto](https://github.com/hexvale-labs/colecto/releases/latest) for your
   system. The builds are not code-signed; [here is how to install them](docs/install.md).
2. Open **Settings → Accounts** and add your IMAP/SMTP or JMAP account.
3. Optional: [host the daemon](docs/docker.md) so mail keeps arriving while your
   devices are off, then point every app at it in **Settings → Servers**.

To build and run from a checkout instead, see [development](docs/development.md).

## Security

Please report vulnerabilities privately through
[GitHub security advisories](https://github.com/hexvale-labs/colecto/security/advisories/new),
not in public issues. See the [security policy](.github/SECURITY.md).

## Documentation

| Document | Covers |
| --- | --- |
| [Installing](docs/install.md) | Downloads for each platform and unsigned builds |
| [Docker hosting](docs/docker.md) | Running the daemon with Docker Compose and connecting to it |
| [Accounts and credentials](docs/accounts.md) | IMAP/SMTP/JMAP setup and credential protection |
| [Daemon](docs/daemon.md) | Remote HTTPS, Unix sockets, supervisors, notifications |
| [Extensions](docs/extensions.md) | Manifests, capabilities and the Wasm sandbox |
| [Development](docs/development.md) | Building, running, logging, tests and releases |
| [Project direction](docs/README.md) | Architecture, terminology and how agents work here |
| [Storage](docs/storage.md) | `colecto.db` records, bounds and legacy import |
| [Alpha roadmap](docs/alpha.md) | What alpha means and what is left |
| [Performance](docs/performance.md) | Resource measurement and baselines |
| [Design](docs/DESIGN.md) and [product](docs/PRODUCT.md) | Visual language and product principles |
| [Prototype design](docs/prototype-design.md) | The original local demo design |
| [Client review](docs/client-review.md), [UX review](docs/ux-review.md) | UI audits and adversarial fixtures |
| [Specs](docs/superpowers/specs/) and [plans](docs/superpowers/plans/) | Approved designs and their implementation status |
| [Agent instructions](AGENTS.md) | Rules for AI agents working in this repository |
