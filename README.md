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
  <a href="https://github.com/hexvale-labs/colecto/pkgs/container/colectod"><img alt="Docker image" src="https://img.shields.io/badge/docker-ghcr.io-2496ed?logo=docker&logoColor=white"></a>
  <img alt="Platforms" src="https://img.shields.io/badge/platforms-Linux%20%7C%20Windows%20%7C%20Android%20%7C%20web-555">
  <img alt="Built with Rust and Tauri" src="https://img.shields.io/badge/built%20with-Rust%20%2B%20Tauri-b7410e?logo=rust">
</p>

<p align="center">
  <a href="https://github.com/hexvale-labs/colecto/releases/latest"><b>Download</b></a> ·
  <a href="docs/self-hosting.md"><b>Self-host with Docker</b></a> ·
  <a href="docs/development.md"><b>Build from source</b></a>
</p>

> [!NOTE]
> Colecto is in alpha. It works with real mail accounts, but storage formats may still
> change between versions. See the [roadmap](docs/roadmap.md).

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
- **Extensions:** the whole UI is built from bundled extensions; third-party panels
  and overlays run in an isolated WebAssembly sandbox
  ([make your own](docs/extensions/guide.md), or try the
  [Oneko cat](docs/extensions/examples/oneko.colectoext))
- **Self-hostable daemon** that keeps receiving mail while every app is closed, with
  [Docker](docs/self-hosting.md), systemd or any other supervisor
- **Automation with guard rails:** scoped API keys and an MCP server, with your
  approval required before anything is sent
- **Encrypted credentials**, protected by your system keychain or a master password
- **Notifications** in the app and on the desktop, positioned where you want them
- **Your accent colour**, on a calm dark interface
- **Platforms:** Linux, Windows, Android and the browser

Have an idea? [Open an issue](https://github.com/hexvale-labs/colecto/issues/new) and
tell us about it.

### Planned

- Read/unread, star, archive, delete and move (in progress)
- Browsing every folder with paging and incremental sync
- First-run onboarding, font choice and running the daemon as a background service
- Android notifications through UnifiedPush from your own daemon
- Conversation threading and inline (`cid:`) images
- Encryption of stored mail
- Hosting several users on one daemon

See the [roadmap](docs/roadmap.md) for status.

### Not planned

- POP3

## Get started

1. [Download Colecto](https://github.com/hexvale-labs/colecto/releases/latest) for your
   system. The builds are not code-signed; [here is how to install them](docs/install.md).
2. Open **Settings → Accounts** and add your IMAP/SMTP or JMAP account.
3. Optional: [host the daemon](docs/self-hosting.md) so mail keeps arriving while your
   devices are off, then point every app at it in **Settings → Servers**.

To build and run from a checkout instead, see [development](docs/development.md).

### Apple builds

Releases no longer include macOS or iOS builds. They may come back, but for now they
do not make sense: they were not signed or notarized, so they were awkward to install
at all, and building them made CI take about twice as long. The maintainer has no
Apple device to test them on and no plans to buy one at the moment.

The macOS and iOS code and setup are still in the repository, so you can build them
yourself on a Mac; see [Apple builds](docs/install.md#apple-builds).

## Security

Please report vulnerabilities privately through
[GitHub security advisories](https://github.com/hexvale-labs/colecto/security/advisories/new),
not in public issues. See the [security policy](.github/SECURITY.md).

## Documentation

Everything is indexed in [docs/README.md](docs/README.md): installing, accounts,
self-hosting, development, architecture, the roadmap, extensions and design.

## License

Colecto is dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option.
