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
 ┌─────────── Svelte UI (app origin, trusted) ───────────┐   ┌─ opaque-origin frames ─┐
 │ shell (ui/src/shell), renderer, extension runtimes;    │◄─►│ extension client parts │
 │ (bundled extensions run as client parts too)           │   │ in Web Workers         │
 └───────────────────────────┬────────────────────────────┘   └────────────────────────┘
                             │ HTTP API (/api/v1), SSE events
 ┌───────────────────────────▼────────────────────────────────────────────────────────┐
 │ Host: auth, extension policy, credentials and service parts (wasmi), API keys, MCP │
 ├────────────────────────────────────────────────────────────────────────────────────┤
 │ Core services: accounts, credential vault, IMAP/SMTP/JMAP, drafts, labels,         │
 │ snapshot cache, Turso storage                                                      │
 └────────────────────────────────────────────────────────────────────────────────────┘
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
An extension's client part runs in a Web Worker inside an opaque-origin sandboxed frame,
with no DOM and no network; it sends component trees that the shell's renderer draws,
and reaches the API only through the shell, with its own credential that the Host checks
on every call. An optional service part runs in a fuel- and memory-limited `wasmi`
runtime without WASI. Bundled extensions use the same runtime. See the [extension contract](extensions/README.md) and
the [extension UI model](extensions/ui-model.md). Rust service parts can use the
[Rust SDK](extensions/rust-sdk.md).

The app frame, docking workspace, account switcher, Settings, command runner, toasts,
bootstrapping, permission prompts, credential controls and recovery are core UI
(`ui/src/shell/` and `ui/src/`), not extensions.

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
catalogs, extension policy and packages, API keys, the send-approval journal, owner
preferences and the optional AI settings and summaries. The OS keyring and password-derived keys stay outside it. Only one
process may open a data directory at a time. Mail snapshots and drafts are not
encrypted at rest. See [storage](storage.md).

The schema is versioned and migrated forward on startup; a database from Colecto alpha,
another program or a newer build is refused. Settings store only what the owner
changed. See [storage](storage.md).

## Platforms

Business logic stays in Rust and presentation in Svelte. Native platform adapters
belong in the Host or the Tauri app, never in the core. Tauri 2 hosts the desktop and
mobile apps; the browser reaches the same Rust service over HTTP. Linux, Windows,
Android and the browser are supported. macOS and iOS still build from source but are
not released or tested (see [installing](install.md#apple-builds)).

On Android and iOS the app also loads `tauri-plugin-colecto-native`
(`crates/colecto-native/`), which carries its own Kotlin and Swift because CI generates
the mobile projects fresh on every build. It claims the Android back gesture while the
page has history to go back through and streams predictive-back progress to it as
`colecto:back` window events, and its one command, `press`, plays the system click
sound and haptics. The capability `native-feedback`
(`crates/colecto-app/capabilities/native.json`) grants only that command, and only to
the phone app's own loopback page (`http://127.0.0.1:*`); nothing else on that page can
reach Tauri. On desktop the plugin does nothing; the desktop page's capability
`desktop-page` (`capabilities/desktop.json`) adds only its own zoom.

### Desktop app without a port

The desktop app runs the mail service in-process and opens **no port** while MCP is off
(`crates/colecto-app/src/desktop.rs`). Phones still use a loopback port, because
Android's WebView does not give custom schemes the request body.

| Piece | How it works |
| --- | --- |
| Page and API | The window loads `colecto://localhost/` (`http://colecto.localhost/` on Windows). Each request goes to the service's router in-process and the whole response comes back; the service sees the authority `colecto.localhost`. |
| Origin | The page's own origin, or no `Origin` at all (WebKitGTK sends none on same-origin requests to a custom scheme), becomes `http://colecto.localhost`; any other origin becomes `null` and mutations are refused. Only this window can reach the scheme. |
| Event streams | `/api/v1/events` and `/api/v1/extensions/events` never stream through the scheme. The page calls the app's `stream` command with a Tauri `Channel`, which passes the text on as it arrives; `close_stream` and any page load end them, so a reload leaves no stream counting as an open window. |
| Owner token | Injected only when `window.origin` is the page's origin, as before. |
| IPC policy | The scheme handler adds `connect-src 'self' ipc: http://ipc.localhost` to the service's CSP so Tauri calls avoid their slow fallback. |
| MCP and API keys | While MCP is on (Settings → API keys), the app listens on `127.0.0.1:${COLECTO_PORT:-4173}`. That port answers only `Host` values `127.0.0.1:<port>` or `localhost:<port>`, only requests with an API key (`Bearer colecto_…`) plus `/api/v1/version`, and translates its own browser origin to the service's. The owner token never works there. Turning MCP off closes it (the Host emits an `mcp` event). |
| Settings → API keys | Shows the port's URL (`window.__COLECTO_API__`, set by the app). |

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
