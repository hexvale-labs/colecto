# Architecture

How Colecto is put together and the rules that keep it that way. For file
locations, see the [documentation index](README.md#where-things-are).

## Goals

- Deep customisability: reading, composing, layouts and workflows can all be changed
  by extensions.
- Mail and account integrity even when an extension fails or misbehaves.
- One codebase for desktop, mobile, browser and a self-hosted daemon, without
  claiming a platform works before it has been tested.

## Layers

```
 ┌──────────────── Svelte UI ────────────────┐   ┌── third-party extensions ──┐
 │ shell + bundled extensions (trusted code) │   │ WebAssembly in wasmi       │
 └─────────────────────┬─────────────────────┘   └─────────────┬──────────────┘
                       │ HTTP API (/api/v1), SSE events         │ Host bridge
 ┌─────────────────────▼────────────────────────────────────────▼─────────────┐
 │ Host: auth, extension policy and sandbox, API keys, approvals, MCP, prefs  │
 ├────────────────────────────────────────────────────────────────────────────┤
 │ Core services: accounts, credential vault, IMAP/SMTP/JMAP, drafts, labels, │
 │ snapshot cache, Turso storage                                              │
 └────────────────────────────────────────────────────────────────────────────┘
```

**Core services** (`colecto-core`) own accounts and credentials, receiving and
storing mail, reading and querying messages, drafts and sending, and settings
persistence. Reading and composing *interfaces* belong to extensions; message
access, draft persistence and delivery belong to the core.

**The Host** (`colecto-host`) wraps the core in one reusable `MailService`. It is run
three ways: `colecto-host` (local browser use), `colecto-daemon` (a server or
background service) and inside `colecto-app` (Tauri). It owns extension discovery,
manifest and API-version checks, capabilities, isolation and recovery. Every request
is validated at this boundary, including requests from bundled extensions. Disabling
an extension never disables core services or recovery controls.

**Extensions** use small, typed, versioned contracts (`colecto-extension-api`, API
version 1). They never touch credential stores, the database, unrestricted networking
or Tauri commands directly. An extension holds every capability its manifest declares
while enabled and none while disabled; the owner consents once, when installing.
Bundled extensions are trusted Svelte code compiled into the app; third-party
extensions run in a fuel- and memory-limited `wasmi` runtime without WASI and can
only emit declarative views or overlay sprites. See the
[extension contract](extensions/README.md). Rust guests can use the
[Rust SDK](extensions/rust-sdk.md) instead of the raw imports.

Only bootstrapping, permission prompts, credential controls and recovery stay outside
the extension composition.

## Mail

Receiving uses IMAP and sending uses SMTP, or JMAP does both. POP3 will not be
supported. The Host polls every connected account and also keeps one push watcher per
account whose last refresh succeeded: IMAP IDLE on a read-only Inbox, or JMAP push
through the session's `eventSourceUrl`. A reported change refreshes that account and
emits a `mail` event. Servers without IDLE or push stay on polling.

Opening a message fetches the raw message from the provider without marking it seen.
HTML is untrusted: it renders in an `<iframe sandbox>` with no scripts, forms or
same-origin access, and remote images wait for the owner unless Settings → Privacy
allows them. Links in mail open in a new window (`<base target="_blank">`); the
desktop app hands `http`, `https` and `mailto` links to the system browser
(`external_link_allowed` in `colecto-host/src/runtime.rs`) and never opens its own
windows for them. Attachments download as `application/octet-stream` so they never render
in the app origin. Details and limits are in [accounts and credentials](accounts.md).

## Storage

One embedded Turso database, `colecto.db`, per data directory holds the account
registry and encrypted credential vault, drafts, local labels, mail snapshots, folder
catalogs, extension policy and packages, API keys, the send-approval journal and owner
preferences. The OS keyring and password-derived keys stay outside it. Only one
process may open a data directory at a time. Mail snapshots and drafts are not
encrypted at rest. See [storage](storage.md).

In alpha, persisted formats may change without migrations; old JSON files from early
prototypes are ignored.

## Platforms

Business logic stays in Rust and presentation in Svelte. Native platform adapters
belong in the Host or the Tauri app, never in the core. Tauri 2 hosts the desktop and
mobile apps; the browser reaches the same Rust service over HTTP. Linux, Windows,
Android and the browser are supported. macOS and iOS builds are produced but unsigned
and untested (see [installing](install.md#experimental-apple-builds)).

On Linux the app sets its GTK program name to `Colecto` so the Wayland `app_id`
matches the `Colecto.desktop` entry every package installs; compositors use that
match to find the window icon.

Wayland is the Linux target. X11 works only as far as Tauri and GTK provide it
without extra code: do not add X11-specific glue, and verify native Linux behaviour
on Wayland first. The `just test native` smoke test runs under Xvfb because CI has
no Wayland compositor; that is test tooling, not X11 support.

## Daemon and devices

One owner may connect several accounts and devices to one daemon. Remote access needs
an explicit HTTPS public origin and an owner bearer token, with TLS terminated by a
reverse proxy. Local Unix sockets use private directories. Multi-user hosting is not
implemented. See [self-hosting](self-hosting.md).

Device synchronization is a later goal. Keep identifiers and storage ownership clear,
but do not build speculative sync code before it is designed.

The local Host can retain UI and owner authentication while proxying API bodies and
SSE to a daemon through an absolute `unix:` socket address. Its persisted
`upstream.json` references the daemon's private owner-token file; scoped callers
cannot acquire that owner authority. Upstream switching and systemd user-service
controls remain local and are refused by remote Hosts and socket daemons.

## Working rules

- Prefer working vertical slices, existing patterns, small modules and explicit
  ownership. Add an abstraction only when real callers need it.
- New crates go under `crates/`; they inherit `version`, `edition`, `publish`,
  `license` and lints from the workspace. Shared dependency versions live once in the
  root `[workspace.dependencies]`. The one exception is `colecto-extension-sys`, which
  does not inherit the workspace's `unsafe_code = "forbid"` because it holds the guest
  SDK's few unsafe FFI lines.
- Dependencies point one way: the Host depends on the core and the extension API; the
  extension API depends on neither. The guest SDK (`colecto-extension`) depends only on
  `colecto-extension-sys` and serde, never on the Host or core, so a Git dependency on
  it stays small. Package and manifest rules live in `colecto-package`, which the Host
  and `cargo-colecto` both use, so the tool and installation never disagree.
- Never describe a planned platform, sandbox property or feature as working.
