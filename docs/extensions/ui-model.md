# Client-side extension UI model (design)

The planned replacement for today's extension system: every extension, bundled or
third-party, runs its client part in a Web Worker on the device and describes its
interface as a component tree that trusted Colecto code renders.

**Status: proposed design, not implemented.** Nothing on this page works yet. The
current system is described in the [extension contract](README.md). This page becomes
the contract, section by section, as each step in [Migration order](#migration-order)
lands; until then, where the two disagree, the current contract describes what runs.

## Goals

| Goal | How |
| --- | --- |
| One model for every extension | Bundled and third-party extensions use the same package, manifest, runtime, credentials and renderer. Bundled ones only differ in shipping with the app (they can be turned off, not removed). |
| Extension code never reaches the owner token, credentials, the app's DOM or another extension's state | Client code runs in a Worker with no DOM, no network and an opaque origin; it talks only to a trusted broker over one message port. |
| Interfaces look like Colecto | Extensions send components, never HTML; the renderer draws them with the existing Svelte components and `ui/src/style.css`. |
| Background work without a window | An optional Wasm service part runs in the Host with today's sandbox. |
| Nothing old to carry | A fresh reset to API 1. Old packages are dropped, not adapted. |

Non-goals for API 1: extension-to-extension imports, shared mutable stores, arbitrary
networking from extensions, extensions replacing core UI (see [Core UI](#core-ui)), and
multiple panes per extension.

## Overview

```
 app origin (trusted)                                    opaque origins (untrusted)
 ┌──────────────────────────────────────────────┐        ┌─────────────────────────────┐
 │ shell: app frame, docking, Settings, Ctrl K, │        │ runtime frame (one per      │
 │ toasts, onboarding and recovery              │        │ extension, sandboxed iframe)│
 │                                              │  port  │  └ Worker: extension client │
 │ renderer: component trees → Svelte           │◄──────►│    part (JS, Wasm or both)  │
 │ broker: one per extension, holds its         │        └─────────────────────────────┘
 │ credential, forwards API calls               │        ┌─────────────────────────────┐
 │                                              │◄──────►│ frame: sandboxed iframe for │
 └──────────────────────┬───────────────────────┘ bridge │ mail bodies and the editor  │
                        │ HTTP API, one event stream     └─────────────────────────────┘
 ┌──────────────────────▼───────────────────────────────────────────────────────────────┐
 │ Host: owner auth, extension credentials and policy, bus, service parts (wasmi)       │
 ├──────────────────────────────────────────────────────────────────────────────────────┤
 │ Core services: accounts, vault, IMAP/SMTP/JMAP, drafts, labels, snapshots, storage   │
 └──────────────────────────────────────────────────────────────────────────────────────┘
```

## Packages and manifest

One package, one manifest and one install consent cover both parts.

### Package layout

A `.colectoext` stays a zip archive with Stored or Deflate entries, read by
`crates/colecto-package/`.

| Entry | Required | Contents |
| --- | --- | --- |
| `manifest.toml` | Yes | The manifest below |
| `client/` | Yes | Everything the client part may load. The `client` field names its entry. |
| `client/<entry>.js` or `client/<entry>.wasm` | Yes | The entry: one classic-script JS bundle, or one Wasm module |
| `client/**/*.wasm`, `client/**/*.json`, `client/**/*.bin` | No | Extra Wasm modules and data files the entry loads with `colecto.file(path)` |
| `client/**/*.html`, `client/**/*.js`, `client/**/*.css` | No | Documents, scripts and styles for `frame` components |
| `service.wasm` (any name, set by `service`) | No | The service part |
| `assets/*.(png\|jpg\|jpeg\|webp\|gif)` | No | Raster images for `image` components and, later, `canvas`. Same rules as today. |

The JS entry is a single bundled classic script (for example `esbuild --bundle
--format=iife`), not an ES module graph. Workers load it from a blob URL, and classic
scripts avoid depending on module-worker support in every webview. A JS entry can mix
in Wasm: it reads `client/parser.wasm` through `colecto.file` and instantiates it
itself. A Wasm entry is instantiated by Colecto's runtime shim with the
[Wasm client ABI](#wasm-client-abi).

Bundled extensions use the same layout. Their source stays in
`ui/src/extensions/<id>/`; the UI build bundles `client/` into the app's static
assets, and the Host embeds their manifests as today. The AI service part is a Rust
crate at `ui/src/extensions/ai/service/` (a workspace member, kept next to its
extension so one package stays one folder), built for `wasm32-unknown-unknown` and
embedded in the Host.

### Manifest

```toml
id = "word-counter"
name = "Word counter"
description = "Counts the words in the open message."
author = "Your name"
api_version = 1
client = "client/main.js"
service = "service.wasm"          # optional
capabilities = ["messages.read", "extensions.bus", "ui.contribute"]

[pane]                            # optional: one workspace pane
icon = "extension"

[settings_page]                   # optional: a section in Settings
title = "Word counter"
icon = "extension"

[bus]
subscribe = ["reader/v1/message_opened"]

[[settings]]                      # unchanged declarative settings
key = "minimum"
label = "Count words longer than"
type = "range"
default = 0
min = 0
max = 10
```

| Field | Change | Rules |
| --- | --- | --- |
| `id`, `name`, `description`, `author`, `api_version` | Kept | As today. `api_version` must be 1. |
| `client` | New, required | Path inside the package under `client/`, ending in `.js` or `.wasm` |
| `service` | New, optional | Path of a Wasm module at the package root |
| `slot` | Removed | Surfaces are declared by `[pane]`, `[settings_page]` and capabilities instead |
| `z_index` | Removed in API 1 | Returns with `ui.canvas` overlays |
| `[pane]` | New | `icon` from the [icon set](#icons). The pane's tab shows the extension `name`. At most one pane per extension. |
| `[settings_page]` | New | `title` (1–32 bytes, must not match a core section title, ignoring case) and `icon`. Without it, an extension with `[[settings]]` gets today's generated page. |
| `navigation` | New, optional | Destinations from `NAVIGATION_TARGETS` this extension opens (`inbox`, `drafts`, `sent`, `compose`). Each has at most one owner; bundled owners win. |
| `capabilities` | Changed list | See [Capabilities](#capabilities) |
| `[bus]`, `[[settings]]` | Kept | As today; bus names keep today's character set including `.` |

### Capabilities

Data capabilities keep today's names and checks. New ones cover UI reach and the Host
services the bundled extensions need. Each one is listed in plain words in the install
warning.

| Capability | Grants | Used by |
| --- | --- | --- |
| `account.read`, `messages.list`, `messages.read`, `messages.label`, `messages.organize`, `drafts.list`, `drafts.read`, `drafts.write`, `messages.send`, `extensions.bus` | Unchanged | All mail extensions |
| `ui.frame` | `frame` components (sandboxed iframes) | Reader, composer |
| `ui.frame.remote` | Frames may load `https:` images. Install warning: servers it loads from can learn when, and possibly what, it shows. | Reader |
| `ui.commands` | Commands and search hits in the command runner | Inbox, reader, composer |
| `ui.app_actions` | Buttons in the app frame's top bar and phone navigation, with a shortcut | Composer (Compose), inbox (folders on phones) |
| `ui.toasts` | Toasts on the core toast surface | Notifications, any extension that wants them |
| `ui.contribute` | Content in another extension's [contribution slots](#contribution-slots) | AI |
| `approvals.notify` | Send-approval events (id, account, sender, subject) and the shell-drawn Reject action | Notifications |
| `releases.read` | The newest stable Colecto release the Host found | Notifications |
| `ai.text` | Summaries and drafted replies through the owner's configured text model | AI |
| `ai.decide` | The decision model for labels | AI service part |
| `ai.configure` | Read and change AI settings; set keys through `secret_field` (write-only) | AI |

`window.overlay` is removed; `ui.canvas` replaces it later. Settings (`settings.read`,
`settings.write` of the extension's own values) and its own [storage](#storage) need
no capability, as `settings.read` does today.

## Client runtime

### Isolation

Each extension with a client part gets, at app start:

1. A hidden runtime frame: an `<iframe sandbox="allow-scripts">` (no
   `allow-same-origin`, so its origin is opaque) loading Colecto's own runtime page
   from the Host. The Host serves that page with
   `default-src 'none'; script-src 'sha256-<runtime>' blob: 'wasm-unsafe-eval'; worker-src blob:; connect-src 'none'; img-src 'none'; style-src 'none'; base-uri 'none'; form-action 'none'`.
2. The shell fetches the package's `client/` files (owner-authenticated) and posts
   them into the frame with one end of a `MessageChannel`.
3. The runtime page creates a dedicated Worker from a blob URL holding Colecto's shim
   plus the extension's entry, and hands it the port. From then on the Worker and the
   shell talk directly over the port; the frame relays nothing.

What this gives the client part:

| Property | Reason |
| --- | --- |
| No DOM | Workers have none |
| No access to the app's `sessionStorage`, `localStorage`, IndexedDB, Cache Storage, cookies or `BroadcastChannel` | The Worker's origin is the frame's opaque origin, not the app's |
| No network: `fetch`, `XMLHttpRequest`, `WebSocket`, `EventSource` are blocked | Blob Workers inherit the frame's CSP, which has `connect-src 'none'` |
| No owner token, no extension credential | Neither is ever posted to the frame or Worker; the [broker](#credentials) holds the credential |
| Can run Wasm | `'wasm-unsafe-eval'` |
| Can start nested Workers | `worker-src blob:`; they inherit the same CSP and die with their parent |

A same-origin Worker would have been simpler, but it would share the app's origin
(IndexedDB, Cache Storage, `fetch('/api/v1/…')`), so the opaque frame is the boundary
and the Worker is defence in depth on top of it.

### Lifecycle

| Event | What happens |
| --- | --- |
| App start, or extension enabled | Runtime frame and Worker start; the shell sends `start` |
| Extension disabled or removed | The shell terminates the Worker and removes the frame; the Host revokes the credential |
| Worker throws at top level, or sends an invalid message | The shell terminates it and shows "*name* stopped" with **Restart** in its surfaces |
| No answer to the watchdog `ping` within 10 s (sent every 5 s) | Same as above, "*name* stopped responding" |
| Page unload | The shell revokes its credentials (best effort) |

Browsers give no way to cap a Worker's memory or CPU. The watchdog only catches a
Worker that stops answering; see [What this does not protect against](#what-this-does-not-protect-against).

### JS API inside the Worker

The shim defines one global, `colecto`, the JS form of the [protocol](#protocol):

| Member | Does |
| --- | --- |
| `colecto.onmessage = (message) => {}` | Receives every shell message (`start`, `event`, `theme`, `account`, `host`, `bus.request`, …) |
| `colecto.send(message, transfer?)` | Sends one protocol message |
| `colecto.file(path)` | Resolves to an `ArrayBuffer` of a file under `client/` |

The shared [client library](#shared-client-library) wraps these; extensions rarely
touch them directly.

### Wasm client ABI

A Wasm entry exports `memory`, `colecto_alloc(len: i32) -> i32` and
`colecto_receive(ptr: i32, len: i32)`, and may import only `colecto.send(ptr: i32,
len: i32) -> i32` and `colecto.file(path_ptr, path_len, out_ptr, out_cap) -> i32`
(bytes written, or -1). Messages are the same UTF-8 JSON as the JS API. The instance
lives as long as the Worker, so a Wasm client keeps its state between messages,
unlike a service part. The Rust SDK implements this ABI.

## Credentials

### Issue

1. The shell, holding the owner token, calls owner-only
   `POST /api/v1/extensions/{id}/credential` for each enabled extension with a client
   part.
2. The Host returns `{token}`: 32 random bytes, hex. It keeps only a SHA-256 verifier in
   memory, bound to the extension ID and its current generation (the one the compiled
   cache already tracks). Tokens are never written to disk.
3. The token stays in that extension's broker object in the shell. It is never placed
   in storage, the DOM, a URL, the runtime frame or the Worker.

### Use and scope

The broker turns a Worker's API call into a request with
`Authorization: Colecto-Extension <token>` and `x-colecto-account`, and **no** owner
bearer. The Host resolves the principal `Extension { id, generation }` and checks, on
every call, that the extension is enabled, the generation is current, the manifest
declares the capability, and the account exists. Owner-only routes (accounts,
credentials, security, policy, API keys, send-request decisions, preferences, the
credential route itself) refuse extension principals. This replaces today's
`x-colecto-extension` routing label, which only worked because the caller also held
the owner token.

The broker also restricts paths to the extension API prefix, but that is only a
convenience: the Host check is the boundary.

### Revoke

| Trigger | Effect |
| --- | --- |
| Disable, remove or reinstall | Generation changes; every token of that extension fails at its next call, and its event stream ends |
| Host restart | All tokens are gone; the shell mints new ones |
| Page unload | `DELETE /api/v1/extensions/{id}/credential` (best effort) |
| More than 8 live tokens for one extension (many windows) | The oldest is dropped |

In local-proxy mode (`upstream.json`), the mint route and the `Colecto-Extension`
scheme pass through to the daemon like other API routes, so the daemon that holds
extension policy issues and checks them. This needs a test in the runtime step.

### Service parts

Service parts never hold tokens. The Host binds their identity to each invocation, as
it does for Wasm guests today.

## Shared client library

A TypeScript library at `ui/src/extension-sdk/` that runs inside Workers. Bundled
extensions import it at build time; third-party authors vendor it from a release tag,
as they pin the Rust SDK today. It is not an extension, holds no shared state between
extensions (each Worker has its own copy), and replaces the data plumbing in
`MailShell.svelte`.

| Area | Replaces in `MailShell.svelte` / `lib/api.ts` | API |
| --- | --- | --- |
| Accounts | `refreshAccounts`, `accounts`, `connected` | `accounts()`, `selected` (kept current by the shell's `account` message) |
| Folders and labels | `folders`, `labels`, `label()` | `folders()`, `labels()`, `label(name, id?, remove?)` |
| Message lists | `loadList`, `syncFromCache`, `folderCache` | `cached(folder)`, `refresh(folder, query?)`, an in-Worker folder cache |
| Messages | `services.reader` | `message(id)`, `content(id)`, `attachment(id, index)` |
| Content cache and prefetch | `contentCache`, `prefetchContent`, `rememberMessages` (150 entries) | `content(id)` with the same LRU, `prefetch(ids)` |
| Drafts and sending | `services.composer` | `drafts()`, `draft(id)`, `saveDraft()`, `send()` |
| Sync | `syncStatus`, `loadSync` | `sync()`, `onSync(fn)` |
| Live events | `subscribeEvents` | `on('mail' \| 'sync' \| 'labels' \| 'approval' \| …, fn)` |
| Bus | `extensionBus`, `setBusAccount` | `bus.request`, `bus.emit`, `bus.handle`, `bus.listen`; calls carry the selected account |
| Settings, storage | `ExtensionSettings` reads | `settings()`, `setSettings()`, `storage.get/set` |
| UI | — | `ui.render(surface, tree)` (diffs against the last tree by node `id` and sends patches), `ui.on(node, kind, fn)`, `toast()`, `commands()`, `appActions()` |

### Event stream

Browsers allow six HTTP/1.1 connections per origin, so one stream per extension would
starve the app. The shell opens **one** stream for all running extensions:
`POST /api/v1/extensions/events` with the owner bearer and the list of extension
tokens. The Host validates each token, filters events per extension, and tags each
event with its recipient; the shell delivers it to that Worker. A revoked token drops
out of the stream.

| Host event | Delivered to extensions with |
| --- | --- |
| `mail` (`{account, ids}`) | `messages.list` |
| `labels` (new: a label changed) | `messages.list` or `messages.label` |
| `sync`, `ready`, `resync` | `account.read` |
| `approval` | `approvals.notify` |
| `navigate` | The extension listed for that destination under `navigation` |
| `extension.settings` | Only the extension whose settings changed |
| `bus.request`, `bus.event` | As today: the target bus, or manifest subscribers |

### Bus

Client parts use the existing Host bus through the broker, so the Host stays the one
place that checks subscriptions and capabilities. Requests to an extension's bus go to
its service part when the manifest has `service`, otherwise to its client part (2 s
reply wait, null without a window, as today). A client part reaches its own service
part with `bus.request` to its own ID, and hears the service part's events on its own
bus without subscribing.

Client-to-client traffic now crosses the Host where today's bundled code calls
functions directly, for example opening a message from the list. If that round trip
shows up in `just perf-check` or on a remote daemon, the fix is an in-browser delivery
path in the broker that applies the same manifest rules, with the Host still
authoritative for service parts.

### Storage

Each extension gets a small key-value store in the Host (`storage.get`, `storage.set`,
`storage.delete`), at most 64 KiB in all, deleted on removal. Notifications keeps the
last announced release there; the AI service part keeps the set of messages it already
labelled (today `ai-autolabel-seen`).

## Surfaces

A surface is a place a tree is shown. The shell owns where surfaces are and the
chrome around them.

| Surface | Declared by | Chrome the shell draws | Notes |
| --- | --- | --- | --- |
| `pane` | `[pane]` | Dock tab with the extension `name` and icon; pane menu | Rendered inside `DockArea`. Never told where it is placed. |
| `pane.header` | `[pane]` | Placed after the pane's tab in the group header; on compact layouts, at the top of the pane | Used for the reader's message tabs |
| `settings` | `[settings_page]` | Settings card, section list entry, crumb, and a line "From *name*" | Bundled sections sit in the Built-in group in today's order; third-party ones in Extensions |
| `dialog` | Any extension with a surface, one at a time | Native modal `<dialog>` sheet with the extension name in its header; Escape always closes | AI reply and label editor |
| Contribution | `ui.contribute` | Clipped to the slot; `title` names the contributor | See [Contribution slots](#contribution-slots) |
| `bottom_nav` | `ui.app_actions` | Phone bottom bar | A `nav` with `placement: "bottom"` is lifted there on compact layouts |

Panes report state the shell needs: `{"t":"state","dirty":true,"busy":false,"back":true}`.
`dirty` makes the shell confirm before switching accounts, reloading or closing the
window; `busy` disables account switching and Settings, as composer saving does
today; `back` means the pane wants the next system back (the shell sends `back` and
falls back to its own handling if the pane answers `{"t":"back.unhandled"}`).

### App actions, commands and toasts

These are data, not trees; the shell draws them with its own components.

```json
{"t":"app_actions","items":[{"id":"compose","label":"Compose","icon":"compose","shortcut":"c","disabled":false}]}
{"t":"commands","items":[{"id":"refresh","name":"Refresh current mailbox","disabled":false}]}
{"t":"command_hits","seq":7,"hits":[{"id":"open:42","title":"Lunch on Friday","hint":"Ada"}]}
{"t":"toast","id":"mail:42","title":"Ada","body":"Lunch on Friday","icon":"mail","duration":6000,"system":true,
 "actions":[{"id":"open","label":"Open","primary":true}]}
```

| Item | Rules |
| --- | --- |
| App action | At most 2 per extension. Shown in the top bar, except while a visible pane renders an `action_ref` to it (so Compose moves into the mailbox header as today). `shortcut` is one lowercase letter or `ctrl+<letter>`, never `ctrl+k`, and fires only outside text fields and dialogs, as today's `c`. |
| Command | At most 64 per extension; `name` at most 96 bytes. Running one sends `{"t":"command","id"}`. Commands replace the whole list on each send. |
| Command search | An extension sends `{"t":"command_search","scope":"inbox"}` once; the runner then sends `{"t":"command_query","seq","query"}` as the owner types and shows at most 6 hits per extension from the newest `seq`. |
| Toast | Title at most 128 bytes, body 512. The surface keeps 4 visible (as today) and labels every toast with the sending extension's name. `id` replaces an earlier toast with the same id from the same extension. Pressing an action sends `{"t":"toast_action","toast","action"}`. `system: true` also shows a desktop notification while Colecto is in the background, if the owner allowed it. |
| Toast core actions | `{"navigate":"settings/send-approvals"}` opens a destination. `{"reject_send":"<request id>"}` (needs `approvals.notify`) is drawn by the shell with the fixed label **Reject** and decided by the shell; extensions can never approve a send. `{"url":"https://…"}` opens in the system browser after the click. |

## Component vocabulary

Every component and prop below is public API 1. Unknown types, props or values reject
the message (see [Limits](#limits)). Strings are always text: the renderer never turns
them into HTML, CSS, URLs or script.

### Node JSON

```json
{"type":"button","id":"reply","label":"Reply","icon":"reply","variant":"tool"}
{"type":"stack","direction":"row","gap":8,"children":[ … ]}
```

| Key | Meaning |
| --- | --- |
| `type` | Component name |
| `id` | Optional; 1–64 bytes `[A-Za-z0-9_.:-]`, unique per surface. Required on nodes that send events or receive patches. |
| `children` | Child nodes, for containers only |
| Other keys | The component's props |

### Containers

| Type | Props | Renders as | Replaces |
| --- | --- | --- | --- |
| `stack` | `direction` (`column`, `row`), `gap` (0, 2, 4, 6, 8, 12, 16, 24), `align`, `justify`, `wrap`, `scroll` (`none`, `y`, `x`) | `div` | Layout wrappers |
| `grid` | `columns` (1–4, or `form`) | `div.form-grid` | AI settings fields |
| `section` | `label` or `labelled_by` (a heading `id`), `busy` | `section` with `aria-label`/`aria-labelledby`, `aria-busy` | Mailbox, reader, composer, settings groups |
| `group` | `label`, `role` (`group`, `toolbar`) | `div role=…` | Reply / Reply all / Forward, composer extension buttons, label filters |
| `toolbar` | — | `header.detail-toolbar` | Reader and composer toolbars |
| `fieldset` | `legend`, `legend_hidden`, `disabled` | `fieldset` | Message labels, compose fields, AI options |
| `form` | `role` (`search`, none) | `form`; Enter or a `submit` button sends `submit` with `{values: {id: value}}` | Search, new label, composer, label editor |
| `nav` | `label`, `items: [{id, label, icon, current, count, disabled}]`, `placement` (`inline`, `bottom`) | `nav` of buttons with `aria-current` | Folder navigation, bottom bar |
| `tabs` | `label`, `items: [{id, label, title, active, closable}]`, `close_all` | Message tab strip | Reader message tabs |

### Text and media

| Type | Props | Renders as |
| --- | --- | --- |
| `heading` | `level` (1–4), `text`, `style` (`title`, `folder`, `day`, `group`) | `h1`–`h4` |
| `text` | `text` or `spans: [{text, style: plain\|strong\|code\|kbd\|muted}]`, `tone` (`default`, `muted`, `danger`, `ok`), `size` (`sm`, `md`), `role` (`status`, `alert`), `live` (`polite`), `title`, `wrap` (`normal`, `pre`), `hidden_visually` | `p` or `span` |
| `badge` | `text`, `tone` | Count badge, nav badge |
| `banner` | `tone` (`info`, `warning`, `error`), `text`, `actions: [button nodes]` | `p.reader-banner` / `.error-box`; `role=alert` for warning and error, `status` for info |
| `avatar` | `name`, `seed` | Initial on a hue from `seed`, `aria-hidden` |
| `icon` | `name`, `size` | Decorative `Icon` |
| `image` | `asset` (a name under `assets/`), `alt`, `width`, `height` | `img` from a blob URL |

### Controls

All controls take `disabled`. Text fields keep their value in the renderer; see
[Local interaction](#local-interaction).

| Type | Props | Events |
| --- | --- | --- |
| `button` | `label`, `icon`, `icon_only`, `variant` (`primary`, `secondary`, `tool`, `text`, `icon`, `chip`, `chip_remove`), `detail` (small second line), `pressed`, `busy` (spinner), `title`, `type` (`button`, `submit`), `copy` (text the renderer copies on press) | `press`; with `copy`, `copied` with `{ok}` |
| `menu_button` | `label`, `icon`, `variant`, `items: [{id, label, disabled}]` | `select` with `{item}` |
| `action_ref` | `action` (`<extension>/<app action id>`), `labelled` | Draws another extension's app action; pressing runs it there |
| `input` | `kind` (`text`, `search`, `email`, `url`, `number`), `label`, `label_hidden`, `placeholder`, `value`, `maxlength`, `min`, `max`, `step`, `autocomplete` (`off`, `email`), `inputmode`, `enterkeyhint`, `spellcheck`, `autocapitalize`, `report` (`change`, `input`), `debounce_ms` (0–1000), `error` | `change` or `input` with `{value}` |
| `textarea` | `label`, `label_hidden`, `placeholder`, `value`, `maxlength`, `rows`, `report`, `debounce_ms` | `change` or `input` |
| `toggle` | `label`, `label_hidden`, `checked`, `style` (`switch`, `checkbox`, `chip`) | `toggle` with `{checked}` |
| `select` | `label`, `value`, `options: [{value, label, hint}]`, `placement` | `change` with `{value}` |
| `segmented` | `label`, `value`, `options: [{value, label}]` | `change` |
| `range` | `label`, `value`, `min`, `max`, `step`, `unit`, `format` (`number`, `percent`) | `change` |
| `file_picker` | `label`, `icon`, `multiple`, `accept`, `max_files`, `max_bytes` | `files` with `[{name, type, size}]` and the bytes as transferred buffers |
| `secret_field` | `label`, `target` (`ai.text`, `ai.decision`), `saved` | `saved` with `{saved}`; the typed value goes from the renderer to the Host and never to the Worker (needs `ai.configure`) |

There is no password input: Colecto never asks for credentials inside extension UI.

### Collections

| Type | Props | Events |
| --- | --- | --- |
| `list` | `label`, `mode` (`static`: `ul`/`li`; `virtual`), `rows: [{key, kind: "row"\|"heading", node}]`, `selected` (a row key), `selection` (`none`, `single`), `arrived` (row keys to animate in) | `select` with `{key}` |
| `message_row` | `sender`, `date`, `date_title`, `subject`, `preview`, `unread`, `density` (`full`, `medium`, `compact`) | Inside a `list` row; renders as today's `button.mail-item` with `aria-pressed` |

A `virtual` list renders all rows below 200 and virtualizes above that, measuring one
row per kind and density. Below the threshold the DOM and accessibility tree match
today's fully rendered list; above it, rows outside the viewport and overscan are not
in the DOM. Lists accept up to 100,000 rows; row changes go through the `rows` patch.

### States and overlays

| Type | Props |
| --- | --- |
| `empty_state` | `image`, `title`, `text`, `hint` (spans) |
| `loading` | `text` (`role=status`) |
| `skeleton` | `lines` (1–6) |
| `dialog` | `title`, `open`, `children`, `footer` (nodes); event `close` |

### Embeds

| Type | Props | Capability |
| --- | --- | --- |
| `frame` | See [frame](#frame) | `ui.frame` |
| `slot` | `name` (`v<n>/<name>`), `context` (JSON, at most 4 KiB) | None to declare one |
| `canvas` | Later | `ui.canvas` |

### Icons

`icon` props take a name from the closed set `Icon.svelte` has when API 1 ships (for
example `inbox`, `mail`, `reply`, `replyAll`, `forward`, `copy`, `check`, `attach`,
`close`, `search`, `refresh`, `compose`, `alert`, `info`, `image`, `text`). The list is
published in the API 1 page; adding an icon is a new API version.

### Considered and left out

| Component | Why not in API 1 |
| --- | --- |
| `split` | Panes are split by `DockArea`; no bundled extension splits inside a pane |
| `link` | No bundled extension links out except through mail (inside a `frame`) and the release toast (a toast `url` action) |
| `password` | See above |
| Custom `select` triggers | Only the core account switcher uses one |

## Protocol

Messages between the shell and a Worker are UTF-8 JSON strings, so the JS and Wasm
paths are identical and every size limit can be checked before parsing. Binary data
(files, attachments, frame bytes) travels as transferred `ArrayBuffer`s next to the
string; the JSON refers to them as `{"$buffer": <index>}`.

### Shell to extension

| `t` | Payload | When |
| --- | --- | --- |
| `start` | `{api: 1, id, theme, account, settings, surfaces: ["pane", …]}` | Once |
| `theme` | Theme tokens | Owner changes Appearance |
| `account` | `{id, name, address, connected, configured}` or `null` | Selected account changes |
| `event` | `{surface, node, kind, value?}` | A meaningful interaction |
| `back` | `{surface}` | System back while the pane said `back: true` |
| `surface` | `{surface, shown}` | A pane becomes visible or hidden |
| `command`, `command_query`, `toast_action`, `app_action` | As above | Owner runs them |
| `slot` | `{slot, owner, context}` | A slot this extension contributes to appears, changes context or goes away (`context: null`) |
| `frame.message` | `{node, data}` | A frame script posted to the bridge |
| `host` | `{name, data}` | A filtered Host event |
| `bus.request`, `bus.event` | As today | Bus delivery |
| `reply` | `{id, ok, data \| error}` | Answer to an extension `call` |
| `ping` | `{n}` | Watchdog |

### Extension to shell

| `t` | Payload |
| --- | --- |
| `render` | `{surface, tree}`: replaces the whole surface |
| `patch` | `{surface, ops: [...]}` |
| `state` | `{surface, dirty, busy, back}` |
| `focus` | `{surface, node}`: honoured only within 1 s after an event from that surface, or when the surface first renders |
| `contribute` | `{slot: "<owner>/<name>", tree \| null}` |
| `dialog` | `{tree \| null}` |
| `app_actions`, `commands`, `command_search`, `command_hits`, `toast`, `toast_dismiss` | As above |
| `download` | `{name, data: {$buffer}}`: the renderer saves it as `application/octet-stream` |
| `call` | `{id, method, path, query?, body?}`: an HTTP API call through the broker |
| `frame.post` | `{node, data}` |
| `pong` | `{n}` |

### Patches

| Op | Shape | Effect |
| --- | --- | --- |
| `set` | `{op: "set", node, props}` | Merges props into the node; `null` removes a prop |
| `replace` | `{op: "replace", node, tree}` | Replaces the node and its children |
| `insert` | `{op: "insert", parent, index, tree}` | Inserts a child |
| `remove` | `{op: "remove", node}` | Removes a node |
| `rows` | `{op: "rows", node, start, delete, insert: [rows]}` | Splices a list's rows |

The renderer applies all ops of one message atomically (an invalid op rejects the
whole message) and batches messages per animation frame.

### Local interaction

These never round-trip to the extension: hover, focus, scrolling, text entry,
keyboard navigation in lists, menus, selects, tabs and dialogs, menu open and close,
list row animations, the refresh spinner, tooltips, copy feedback timing and toast
timers. Text fields keep their value in the renderer and report it on `change`, or on
`input` after `debounce_ms` when `report` is `input`. A patch that sets `value`
replaces the field's text, so the library never echoes values back.

## Theme tokens

Settings → Appearance (core) produces the tokens. The shell sends them in `start` and
in a `theme` message whenever they change. Components are styled by the renderer
already; tokens matter for `frame` and `canvas` content and for choices like layout.

```json
{"scheme":"dark",
 "colors":{"ground":"#…","bg":"#…","sunk":"#…","raise":"#…","control":"#…","float":"#…",
           "line":"#…","line_soft":"#…","ink":"#…","ink_2":"#…","ink_3":"#…",
           "accent":"#…","accent_hi":"#…","accent_ink":"#…","select":"#…",
           "online":"#…","danger":"#…","danger_bg":"#…"},
 "font":{"family":"Mona Sans Variable","size":14,"scale":1},
 "density":"pointer",
 "reduced_motion":false}
```

| Token | Source |
| --- | --- |
| `colors` | The `:root` variables in `ui/src/style.css`, resolved to `#rrggbb` (the accent from the owner's choice) |
| `font` | Settings → Appearance font and text size (`lib/typography.ts`) |
| `density` | `touch` on coarse pointers, where text reads one size up (`TOUCH_SCALE`); otherwise `pointer` |
| `reduced_motion` | `prefers-reduced-motion` |

Colecto has one dark theme today. `scheme` is always `dark` until a light theme exists;
the field is there so frames can follow it later without an API change.

## Escape hatches

### Frame

A `frame` is an `<iframe>` with `sandbox` and never `allow-same-origin`, so its origin
is opaque. The renderer builds its document; the extension supplies content.

| Prop | Meaning |
| --- | --- |
| `title` | Required accessible name |
| `doc` | The body HTML as a string (at most 8 MiB), for example a mail body |
| `scripts` | Paths of `client/**/*.js` files to run in the frame; each is inlined with a per-frame nonce |
| `styles` | Paths of `client/**/*.css` files, inlined |
| `remote_images` | `https:` images allowed; needs `ui.frame.remote` |
| `height` | `fill` (today's `max(320px, 100cqh − 64px)`) or a number of pixels |
| `designed` | Light sheet for mail that paints its own colours, as today |

| Frame kind | `sandbox` | CSP |
| --- | --- | --- |
| No `scripts` (mail bodies) | `allow-popups allow-popups-to-escape-sandbox` | `default-src 'none'; style-src 'unsafe-inline'; img-src data:` plus `https:` with `remote_images` |
| With `scripts` (rich-text editor) | adds `allow-scripts` | `default-src 'none'; script-src 'nonce-<random>'; style-src 'unsafe-inline'; img-src data: blob:`; no `connect-src`, so no network |

Links keep `<base target="_blank">`; the desktop app hands them to the system browser
as today. A frame with scripts gets a bridge: its scripts call
`parent.postMessage(data, "*")` and receive `message` events; the renderer accepts
messages only from that frame's `contentWindow`, at most 256 KiB each, and relays them
to the Worker as `frame.message`. The extension answers with `frame.post`. Because only
nonce-tagged scripts run, inline handlers in quoted mail inside the editor do not.

### Canvas

Planned for the Oneko rewrite, not part of API 1 unless it lands before API 1 ships.
The intended shape: `ui.canvas` gives a `canvas` component or a full-window overlay
(`pointer-events: none`, `z_index` 1–10000 from `ui/src/lib/layers.ts`); the renderer
transfers an `OffscreenCanvas` to the Worker, which draws at its own pace, and sends
pointer position at most once per animation frame. Where `OffscreenCanvas` is missing,
the Worker sends `ImageBitmap` frames instead. Sprites come from `assets/`.

## Contribution slots

Today's bundled-only reader slots (`actions`, `aboveBody`, `ui/src/lib/reader-slots.ts`)
become a general mechanism:

1. Any extension puts a `slot` node in its tree, for example
   `{"type":"slot","id":"ai","name":"v1/actions","context":{"message_id":"42"}}`. The
   slot's full name is `<owner id>/<name>`, versioned like bus names, and listed in
   [Bundled bus surfaces](bundled-bus.md) for bundled owners.
2. Extensions with `ui.contribute` that want it receive `slot` messages with the
   context, and answer `contribute` with a tree, or `null` to leave it empty.
3. The renderer draws each contribution inside the slot, clipped to it, in a stable
   order (bundled first, then by install time), at most 4 per contributor per slot.
   Events go to the contributor, never to the slot owner.

The reader's `reply(body)` callback becomes a bus request,
`reader/v1/reply {message_id, body}`, which the reader answers by asking the composer
to open the reply.

| Slot | Context | Filled by |
| --- | --- | --- |
| `reader/v1/actions` | `{account, message_id}` | AI: the AI menu button |
| `reader/v1/above_body` | `{account, message_id}` | AI: the saved summary |
| `composer/v1/actions` | `{}` | Replaces `composer/v1/add_button`; contributors read the draft with `composer/v1/get_draft` |

## Service part

The optional `service` module runs in the Host (`colecto-host`, `colecto-daemon` or
the app's built-in service) whether or not a window is open. It keeps today's sandbox:
`wasmi`, no WASI, a fresh instance per invocation, the existing imports
(`colecto.action`, `colecto.request`, `colecto.emit`) and limits.

| Invocation | Action JSON | Needs |
| --- | --- | --- |
| New mail | `{"event":"mail","account","ids":[…]}` | `messages.list` |
| Bus request to its own bus | `{"event":"bus.request","from","name","payload"}` | `extensions.bus`, or none when `from` is its own client part |
| Bus event it subscribes to | `{"event":"bus.event","bus","name","payload"}` | `extensions.bus` |

Its `colecto.request` operations are today's (`messages.read`, `messages.label`, …,
`settings.read`, `bus.request`, `bus.emit`) plus `storage.get/set/delete`, `ai.decide`
and `ai.text`. It never renders anything; it tells its client part about results with
`bus.emit` on its own bus.

AI auto-labels move from `crates/colecto-host/src/ai.rs` into the AI service part: it
receives `mail`, reads the new messages, applies the owner's regex rules itself, asks
`ai.decide` when a rule says so, and labels through `messages.label`. Provider URLs and
keys stay in the Host; the service part never sees them.

## Limits

| Limit | Value | Notes |
| --- | --- | --- |
| Package | Today's: 16 MiB compressed, 48 MiB uncompressed, 8 KiB manifest, 256 assets of 8 MiB | `crates/colecto-package/src/lib.rs` |
| Client entry | 16 MiB | Same as today's module limit |
| Files under `client/` | 256, each at most 16 MiB | |
| Message from a Worker | 4 MiB of JSON, plus 32 MiB of transferred buffers | Larger messages stop the extension |
| Nodes per surface | 5,000, not counting list rows | |
| Tree depth | 32 | |
| Rows per list | 100,000; each row at most 32 nodes | |
| Patch ops per message | 2,000 | |
| `text` string | 16 KiB; labels, titles and options 128 bytes | Today's view text and action label limits |
| Pending `call`s | 64 per extension | |
| Commands, app actions, toasts | 64, 2, 4 visible in all | |
| Contributions | 4 per contributor per slot | Today's four composer buttons per caller |
| Dialogs | 1 open per extension | |
| Frame document | 8 MiB; bridge messages 256 KiB | |
| Storage | 64 KiB per extension | |
| Watchdog | Ping every 5 s, stop after 10 s without an answer | |
| Bus | Unchanged (32 KiB payload, 32 subscriptions, 2 s reply wait, …) | [Extension bus](README.md#extension-bus) |
| Service part | Unchanged: 32 MiB memory, 10,000,000 fuel, eight requests per invocation, two at once | |
| Installed third-party packages | 32 | Unchanged |

## Security boundaries

### What protects what

| Asset | Boundary |
| --- | --- |
| Owner token | Lives only in the app origin's `sessionStorage` and shell memory; extension requests never carry it |
| Mail account credentials, vault, master password | Core services and core Settings → Accounts only; no extension route or component touches them |
| AI provider keys | Host only; `secret_field` sends them from the renderer to the Host, and nothing reads them back |
| The app's DOM and storage | Opaque-origin runtime frame and Worker; frames are opaque too |
| Other extensions' state | Separate Workers and frames; only the bus and slots connect them, and the Host checks the bus |
| Mail data | Per-extension credential checked by the Host on every call against declared capabilities and enabled state |
| Exfiltration | Workers and script frames have no network; remote images need `ui.frame.remote`; links need a click |
| Being mistaken for Colecto | Pane tabs, settings pages, dialogs, toasts and contributions all carry the extension's name in trusted chrome; settings titles cannot copy core ones |
| Sending mail | `messages.send` as today; send approvals are decided only in core UI, and toasts can only offer a shell-drawn Reject |

### What this does not protect against

- An extension misusing data it is allowed to read, or lying about its purpose. The
  install warning says so, as today.
- Denial of service. Workers and frames run in the webview's renderer process; a
  client part can use a lot of CPU or memory and slow down or crash the window. The
  watchdog only catches a stuck message loop. If a crash repeats, the core recovery
  screen offers **Start without extensions**.
- Spoofing inside an extension's own area. A pane or frame can draw anything,
  including something that looks like a Colecto dialog. Colecto never asks for
  passwords or approvals inside extension areas, and says so in the docs.
- Side channels between Workers in one process. Colecto does not enable cross-origin
  isolation, so `SharedArrayBuffer` and high-resolution timers stay unavailable, but
  this is not a guarantee against speculative-execution attacks.
- Bugs in the browser engine. Isolation relies on the webview being up to date:
  WebKitGTK on Linux, WebView2 on Windows, Android System WebView.
- The service part keeps today's caveats: an in-process Wasm sandbox, not OS isolation,
  and not a security audit.

## Core UI

These leave `ui/src/extensions/` for `ui/src/shell/` (or stay in `ui/src/` where they
already are). There is exactly one of each and none can be turned off.

| Core part | From |
| --- | --- |
| App frame: top bar, logo and wordmark, back handling (`lib/back.ts`) | `MailShell.svelte` |
| Docking workspace (`DockArea.svelte`, `lib/layout.ts`) hosting extension panes | `MailShell.svelte` |
| Account switcher and selected account; the Accounts pane as a shell pane the owner can hide | `MailShell.svelte`, the `accounts` manifest |
| Settings card and section list; core sections Accounts, Connection, Extensions, API keys, Send approvals, Appearance | `MailShell.svelte`, `ExtensionSettings.svelte` |
| Command runner (`CommandRunner.svelte`) | `MailShell.svelte` |
| Onboarding, connect with token, "can't reach your mail", "can't be loaded", other recovery screens | `App.svelte` and friends |
| Toast surface (`Toasts.svelte` and its store) | `extensions/notifications/` |
| Renderer, broker, runtime frame, shared library | New |

Removed: the `accounts` and `mail-shell` manifests and their entries in
`crates/colecto-host/src/extensions.rs`, the "Mail workspace disabled" state and its
recovery button in `App.svelte`, and the shell's import of the notifications store.

## Bundled extensions on the new system

| Extension | Owns | Settings section | Capabilities | Bus (new names marked *) |
| --- | --- | --- | --- | --- |
| `notifications` | What to notify about: new mail, send approvals, new releases; shows them as toasts | Notifications: on/off, mail, approvals, releases | `ui.toasts`, `approvals.notify`, `releases.read`, `messages.list`, `account.read`, `extensions.bus` | `v1/notify` |
| `ai` | Summaries, drafted replies, auto-labels (service part) | AI, as today | `ai.text`, `ai.decide`, `ai.configure`, `messages.read`, `messages.list`, `messages.label`, `ui.contribute`, `extensions.bus` | Calls `reader/v1/reply`* |
| `inbox` | Message list, folders and labels navigation, search, sort, selection, refresh and sync status | Mail: density, date headings, prefetch | `account.read`, `messages.list`, `messages.label`, `messages.organize`, `drafts.list`, `ui.commands`, `ui.app_actions`, `extensions.bus` | `v1/selected` (adds optional `folder`, `subject`), `v1/listed`* `{account, folder, ids}` for prefetch |
| `reader` | Message view, tabs, body in a `frame`, remote images, banners, labels on the message, slots | Privacy: remote images | `messages.read`, `messages.list`, `messages.label`, `messages.organize`, `ui.frame`, `ui.frame.remote`, `ui.commands`, `extensions.bus` | Today's four, `v1/reply`*, slots `v1/actions`, `v1/above_body` |
| `composer` | Editor (in a `frame`), drafts, sending, attachments, buttons from other extensions | — | `drafts.read`, `drafts.write`, `messages.send`, `messages.read`, `ui.frame`, `ui.app_actions`, `ui.commands`, `extensions.bus` | Today's, `v1/open`* `{draft_id}` or `{draft}`, slot `v1/actions` replacing `v1/add_button` and `v1/remove_button` |
| `oneko` (example) | Cat overlay | Speed, opacity, stopping distance | `ui.canvas` | — |

The inbox owns destinations `inbox`, `drafts` and `sent`; the composer owns `compose`.
The default layout keeps the mailbox on the left and the reader on the right; the
composer's pane starts in the reader's group and comes forward when a draft opens.
The owner preferences these extensions take over (`list_density`, `date_groups`,
`prefetch`, `remote_images`, `notifications`, `notified_release`) become their
`[[settings]]` or storage and start from defaults.

## Fresh reset to API 1

| What | Action |
| --- | --- |
| `crates/colecto-extension-api/api-versions.json` | One entry: version 1, `breaking: false`, `released_in: null`, every capability above |
| `api-support.json` | Stays empty |
| `docs/extensions/api-versions/API-1.md`, `API-2.md`, `API-3.md` | Deleted; a new `API-1.md` describes this model |
| `README.md`, `EXAMPLE.md` there | Kept, updated; the rule that every later change adds the next version stays, and so does discontinuation |
| Panel view and overlay sprite ABI, `/extensions/{id}/run`, `overlay.rs`, panel and overlay paths in `sandbox.rs`, `ExtensionPanel.svelte`, `GuestOverlay.svelte`, `OverlayLayer.svelte` | Deleted |
| `docs/extensions/examples/connection-panel` | Deleted; replaced by a small JS example pane |
| Oneko | Old source deleted; its sprite sheet kept; rewritten on `canvas` in the last step. `just build oneko` and its checks are removed until then. |
| Rust SDK (`colecto-extension`, `colecto-extension-sys`) | `View`, `PanelEvent`, `Overlay`, `Frame` and `export! { panel, overlay }` deleted; new client-part support over the Wasm client ABI and service-part support over the kept imports |
| `cargo colecto` | Builds the client Wasm and the service Wasm, checks both against the manifest, packages `client/` and `service` |
| Stored data | On startup, third-party packages installed under the old system (old policy version, or a manifest that no longer parses) are dropped with their settings and a log line, instead of failing to start. Unknown old preference fields are ignored. |
| `.github/release-notes/pending.md` | A short note: the extension system was rebuilt, old extensions no longer load, and Mail, Privacy and Notifications settings start from defaults |

There are no adapters, detection or migration for old packages.

## Migration order

Each step is its own pull request against `develop`, shippable alone: `just verify`,
`just test browser`, `just perf-check`, docs updated in the same change.

1. **This design.** Wait for the maintainer's approval.
2. **Core UI out.** Move the [core parts](#core-ui) to `ui/src/shell/`; remove
   `accounts` and `mail-shell`. Not-yet-migrated bundled extensions keep running as
   trusted Svelte inside the shell until their own step; this interim path is internal
   and never loads third-party code.
3. **Runtime and renderer.** Runtime frame, Workers, broker, credentials, multiplexed
   event stream, renderer and vocabulary, theme protocol, `frame`, shared library,
   service part changes, the reset to API 1, the JS example, SDK and `cargo colecto`.
   May split into two pull requests (runtime with credentials; renderer with `frame`)
   if review needs it.
4. **Notifications**, then **AI**, then **inbox**, then **reader**, then
   **composer**, one pull request each, each deleting the Svelte and the
   `MailShell.svelte` plumbing it replaces.
5. **Oneko** on `canvas`.

### Acceptance for each migrated extension

| Check | How |
| --- | --- |
| Looks the same | Playwright screenshots of the old and new version at 390 px and 1280 px, with `colorScheme` `light` and `dark` (today both render the dark theme; light only changes mail content), compared within a small pixel tolerance. Every intentional difference is listed in the pull request. |
| Same accessibility tree | `toMatchAriaSnapshot` against snapshots taken from the old version: roles, names, states and focus order |
| Tests | Every existing spec that covers it is ported; any deleted test is named in the pull request with why |
| Performance | `just perf-check` within 10% of the baseline on every metric, comparing only matching metadata |
| Platforms | See below |

## Platform verification

Each step says which of these it verified and on what.

| Feature | Browser (Chromium in Playwright) | Linux app (WebKitGTK) | Windows app (WebView2) | Android (System WebView) |
| --- | --- | --- | --- | --- |
| Opaque-origin runtime frame with blob Worker | Automated | Manual | Manual | Manual |
| Blob Worker inherits the frame's CSP (`connect-src 'none'` blocks `fetch`) | Automated test that `fetch` fails | Manual | Manual | Manual |
| `'wasm-unsafe-eval'` and Wasm in the Worker | Automated | Manual | Manual | Manual |
| `MessageChannel` port passed through an opaque frame | Automated | Manual | Manual | Manual |
| Mail `frame` without scripts, links to the system browser | Automated (today's specs) | Manual (navigation allowlist in `runtime.rs`) | Manual | Manual |
| Editor `frame` with nonce scripts and bridge | Automated | Manual | Manual | Manual |
| `OffscreenCanvas` in a Worker (Oneko step) | Automated | Manual; fall back to `ImageBitmap` if missing | Manual | Manual |

`native_navigation_allowed` must also admit the runtime page; WebKitGTK asks about
subframe loads.

## Expected visible differences

To be confirmed or removed in each step's pull request.

| Difference | Why | Step |
| --- | --- | --- |
| Toasts show the sending extension's name | Trusted attribution; today only third-party bus toasts are prefixed | Notifications |
| Toast position and desktop notifications move to Settings → Appearance | They configure the core toast surface, not what to notify about | Notifications |
| Settings pages from extensions show "From *name*" | Trusted attribution | Notifications |
| Extension dialogs show the extension name in their header | Trusted attribution | AI |
| AI contributions have a tooltip naming AI | Trusted attribution | AI |
| List density and date headings move from Appearance to Mail | The inbox owns Settings → Mail | Inbox |
| Bundled names become their tab names: "Mailbox" → "Mail", "Message reader" → "Reader" | Pane tabs show the extension name | Inbox, reader |
| The composer has its own pane tab next to Reader | One pane per extension | Composer |
| Mail, Privacy and Notifications settings start from defaults once | Fresh reset | Runtime |

## Open questions

Decisions the maintainer did not cover that change the work. Each has a proposal; this
pull request waits for answers.

1. **Rich text in the composer.** Today's composer is a plain `textarea`, but the
   editor is meant to be rich text in a `frame`. Proposal: the composer step moves the
   plain-text editor into a script `frame` that looks and behaves the same, and rich
   text follows in its own change, so the "looks the same" bar stays meaningful.
2. **Attribution marks** in the table above change pixels and the accessibility tree.
   Proposal: accept them as intentional differences, because otherwise a third-party
   toast or dialog could pass as Colecto's.
3. **Composer pane.** Proposal: its own pane in the reader's group (one pane per
   extension), rather than a way for one extension to take over another's pane.
4. **Bus round trip.** Proposal: everything through the Host first; add the
   in-browser fast path only if measurements need it.
5. **Toast settings in Appearance** (position, desktop notifications), as in the table
   above.
