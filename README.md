<p align="center">
  <img src="docs/branding/logo-square.svg" alt="" width="96" height="96">
</p>

<h1 align="center">Colecto</h1>

<p align="center">
  <b>A quiet, keyboard-friendly email client for IMAP/SMTP and JMAP.</b><br>
  Rust mail engine, native multi-platform Flutter interface,<br>
  and a daemon you can host yourself.
</p>

<p align="center">
  <a href="https://github.com/kogical/colecto/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/kogical/colecto?sort=semver&display_name=tag&label=release&color=a2d6b1"></a>
  <a href="https://github.com/kogical/colecto/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/kogical/colecto/ci.yml?branch=main&label=CI"></a>
  <a href="https://github.com/kogical/colecto/pkgs/container/colectod"><img alt="Docker image" src="https://img.shields.io/badge/docker-ghcr.io-2496ed?logo=docker&logoColor=white"></a>
  <img alt="Platforms" src="https://img.shields.io/badge/platforms-Linux%20%7C%20Windows%20%7C%20macOS%20%7C%20Android%20%7C%20iOS%20%7C%20Web-555">
  <img alt="Built with Rust and Flutter" src="https://img.shields.io/badge/built%20with-Rust%20%2B%20Flutter-b7410e?logo=rust">
</p>

<p align="center">
  <a href="https://github.com/kogical/colecto/releases/latest"><b>Download</b></a> ·
  <a href="docs/self-hosting.md"><b>Self-host</b></a> ·
  <a href="docs/architecture.md"><b>Architecture</b></a> ·
  <a href="docs/README.md"><b>Docs</b></a> ·
  <a href="docs/roadmap.md"><b>Roadmap</b></a>
</p>

> [!NOTE]
> Colecto is in beta. It works with real mail accounts, and updates migrate your data
> forward; going back to an older version is not supported.

## Why Colecto

- **Your mail, your server.** Run `colecto-daemon` yourself and it keeps receiving mail
  while every app is closed. Or skip it and use the standalone desktop/mobile app on its
  own via the in-process `colecto-ffi` library—no loopback HTTP port required.
- **Native on every platform.** One shared Flutter app rendered with each platform's own
  design system: Colecto (`shadcn_ui`) on Linux and Web, Fluent on Windows, macOS UI on
  macOS, Material 3 (with Material You) on Android, and Cupertino on iOS.
- **No extensions, no plugin runtime.** Every feature is built cleanly as a first-party,
  toggleable module—no Wasm sandbox, plugin ABI, or third-party extension loader to
  maintain or secure.
- **Safe by default.** HTML mail is sanitized in Rust with `ammonia`, remote images wait
  for your OK, and automation never sends anything without your approval.

## Features

### Mail

- **IMAP/SMTP and JMAP**, side by side, with as many accounts as you like (never POP3)
- **New mail in seconds** through IMAP IDLE and JMAP push, with polling as a fallback
- **Reply, reply all and forward**, with attachments in both directions
- **Search** across folders with server-side filters (`from:`, `to:`, `is:`, `after:`, `before:`)
- **Local drafts, labels, and offline cache**, kept in a single embedded Turso database (`colecto.db`)

### Interface

- **Per-platform native design systems** (switchable in **Settings → Appearance → Design**):
  Colecto (`shadcn_ui`), Fluent, macOS, Material 3, and Cupertino
- **Adaptive 3-tier layout** that scales from compact phone screens to wide desktop monitors
- **Keyboard first:** command palette (<kbd>Ctrl</kbd>+<kbd>K</kbd>), <kbd>C</kbd> to
  compose, <kbd>J</kbd>/<kbd>K</kbd> navigation, and zoom controls
- **Built-in Demo mode** to explore the UI and test layouts without configuring an account

### Privacy and safety

- **Sanitized HTML mail** via Rust (`ammonia`), with remote images blocked by default and
  rendered natively without keeping idle WebViews alive
- **Encrypted credentials**, sealed in an XChaCha20-Poly1305 vault protected by your
  system keychain or an Argon2id master password
- **Automation with guard rails:** scoped API keys and an MCP server (`colecto-mcp`),
  with your explicit approval required before anything is sent

### Hosting and platforms

- **Standalone local mode** via `colecto-ffi` (`dart:ffi`) with zero loopback TCP sockets
- **Self-hostable daemon** (`colecto-daemon`) serving the HTTP/SSE API, MCP endpoint, and
  Flutter Web UI, packaged for [Docker](docs/self-hosting.md) and systemd
- **Linux, Windows, macOS, Android, iOS, and Web**

Have an idea? [Open an issue](https://github.com/kogical/colecto/issues/new) and tell us
about it.

## Get started

1. [Download Colecto](https://github.com/kogical/colecto/releases/latest) for your
   system. The builds are not code-signed; [here is how to install them](docs/install.md).
2. Open **Settings → Accounts** and add your IMAP/SMTP or JMAP account.
3. Optional: host `colecto-daemon` so mail keeps arriving while your devices are off, then
   point any Colecto app at it.

### Try from a checkout

```sh
just run web    # builds Rust + Flutter Web and runs colecto-daemon on http://127.0.0.1:4173
just run demo   # runs the native Flutter app on Linux with built-in demo data
just run daemon # runs colecto-daemon standalone on 127.0.0.1:4173
```

See [development](docs/development.md) for prerequisites, building `colecto-ffi`, and
running tests.

### Host the daemon

You need a domain name pointing at a server, with ports 80 and 443 open.

```sh
mkdir colecto && cd colecto
curl -fsSLO https://raw.githubusercontent.com/kogical/colecto/main/packaging/docker/compose.yaml
echo "COLECTO_DOMAIN=mail.example.org" > .env
docker compose up -d
docker compose exec colecto cat /data/access-token
```

That starts `colecto-daemon` behind [Caddy](https://caddyserver.com), which gets a TLS
certificate for you, and prints the access token to paste into the browser or the app.
[Self-hosting](docs/self-hosting.md) covers credentials in a container, configuration,
and other supervisors.

## Documentation

Everything is indexed in [docs/README.md](docs/README.md).

| Topic | Page |
| --- | --- |
| Install | [Installing](docs/install.md) |
| Run a server | [Self-hosting](docs/self-hosting.md) |
| Accounts and credentials | [Accounts](docs/accounts.md) |
| Architecture & UI design | [Architecture](docs/architecture.md), [UI design](docs/ui-design.md), [Decisions](docs/decisions.md) |
| Storage | [Storage](docs/storage.md) |
| Hack on Colecto | [Development](docs/development.md) |

## Security

Please report vulnerabilities privately through
[GitHub security advisories](https://github.com/kogical/colecto/security/advisories/new),
not in public issues. See the [security policy](.github/SECURITY.md).

## License

Colecto is dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option.
