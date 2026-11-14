# Extension contract

The exact rules for extension packages, manifests, capabilities, credentials, the client
runtime, the service part and the bus, as they work today (API 1). For a walkthrough,
start with [Make your own extension](guide.md). The [extension UI model](ui-model.md)
is the full design, including the parts still being built; what each API version adds is
in [API versions](api-versions/README.md).

## How an extension runs

| Part | Where it runs | Written in | Talks through |
| --- | --- | --- | --- |
| Client part (`client`, required) | A Web Worker on the device, inside an opaque-origin sandboxed frame | JavaScript, Wasm, or both | One `MessagePort` to the shell: the [protocol](ui-model.md#protocol) |
| Service part (`service`, optional) | The Host (`colecto-host`, `colecto-daemon` or the app's built-in service), without a window | Wasm only | The [service ABI](#service-part) |

Extensions never write HTML. A client part sends component trees that trusted Colecto
code renders (see [Components](#components)); everything around them, such as pane tabs
and settings pages, is shell chrome that names the extension.

Bundled extensions (`ui/src/extensions/<id>/`) are moving to the same runtime one at a
time (see [migration order](ui-model.md#migration-order)). Until its own step, a bundled
extension still runs as trusted Svelte in the app origin, has no `client` entry, and
cannot be installed or removed. The app frame, docking workspace, account switcher,
Settings, command runner, toast surface and recovery screens are core UI in
`ui/src/shell/` and cannot be turned off.

## Manifest

```toml
id = "hello-pane"
name = "Hello pane"
description = "Shows which account is selected and counts presses of a button."
author = "Your name"
api_version = 1
client = "client/main.js"
capabilities = ["account.read"]

[pane]
icon = "extension"
```

| Field | Required | Rules |
| --- | --- | --- |
| `id` | Yes | Lowercase ASCII letters, digits and hyphens; also the extension's bus name |
| `name`, `description`, `author` | Yes | Not empty. Author names are self-reported, not verified |
| `api_version` | Yes | A version this Colecto supports (see [API versions](api-versions/README.md#how-the-host-uses-them)) |
| `client` | Yes (third-party) | A `.js` or `.wasm` file under `client/` in the package |
| `service` | No | A `[a-z0-9-]+.wasm` module at the package root |
| `capabilities` | Yes | See [Capabilities](#capabilities); unknown or duplicate entries are rejected |
| `[pane]` | No | `icon` from the [icon set](#icons). One workspace pane; its tab shows `name` |
| `[settings_page]` | No | `title` (1–32 bytes, not a core section title such as Accounts or Extensions, ignoring case) and `icon`. A Settings section the extension draws |
| `navigation` | No | Destinations it opens: `inbox`, `drafts`, `sent`, `compose` |
| `[bus]` | No | `subscribe = ["<bus>/<name>", …]`, needs `extensions.bus`; see [Extension bus](#extension-bus) |
| `[[settings]]` | No | Declarative owner settings; see [Extension settings](#extension-settings) |

Unknown fields are rejected. `slot`, `z_index` and `window.overlay` belonged to the old
panel and overlay system and are gone.

## Packages

A `.colectoext` is a zip archive with Stored or Deflate entries, read by
`crates/colecto-package/` (shared by the Host and `cargo colecto`).

| Entry | Required | Limit |
| --- | --- | --- |
| `manifest.toml` | Yes | 8 KiB |
| `client/**` | Yes; must contain the `client` entry | 256 files, each at most 16 MiB; paths of up to eight `[A-Za-z0-9_.-]` segments not starting with `.`, ending in `.js`, `.wasm`, `.json`, `.bin`, `.html` or `.css` |
| The `service` module | Only when declared | 16 MiB; must be valid Wasm with only the [service imports](#service-part) |
| `assets/*.png|jpg|jpeg|webp|gif` | No | 256 flat names `[a-z0-9-]+`, 8 MiB each, magic bytes must match; SVG is rejected |

The whole archive is at most 16 MiB, 48 MiB uncompressed, with no entry above 100:1
compression; sizes are checked before and while inflating. At most 32 third-party
packages are installed. Anything else in the archive, a missing client entry, or a
service module the manifest does not declare (or one it declares but lacks) rejects
the package.

Run `just build extension` to package the JavaScript example in
`docs/extensions/examples/hello-pane/` as `target/extensions/hello-pane.colectoext`.

## Install, enable and remove

Settings → Extensions installs a package (pick it, or drop it anywhere on the page).
`POST /api/v1/extensions/review` validates it and returns `{manifest}` first, so a
five-second warning can list every capability in plain words; it says that the
permission system cannot stop an extension from lying about its purpose or misusing
data it may read. Installing enables the extension with every capability it declares,
on every account. Disabling turns all of them off; there are no individual grants.
Removing deletes the package and its settings; saved mail is never removed.

Installing an ID that already exists, including a bundled one, is refused. Policy and
packages live in the Host database (policy version 2). On startup, packages from the
old extension system (policy version 1) and packages that no longer load are dropped
with their settings and a log line, instead of keeping Colecto from starting.

## Capabilities

An enabled extension holds every capability it declares, on every account; a disabled
one holds none. The Host checks enabled state, generation and declared capability on
every call.

| Capability | Grants |
| --- | --- |
| `account.read` | Account connection status |
| `messages.list`, `messages.read` | List messages; read them and their attachments |
| `messages.label`, `messages.organize` | Change local labels; flag, move, archive, delete |
| `drafts.list`, `drafts.read`, `drafts.write` | Drafts |
| `messages.send` | Send mail (with `forward_of`, also `messages.read`) |
| `extensions.bus` | Talk to other extensions by name |
| `ui.frame`, `ui.frame.remote` | `frame` components; frames that may load `https:` images |
| `ui.commands`, `ui.app_actions`, `ui.toasts`, `ui.contribute` | Commands, top-bar buttons, toasts, content in other extensions' slots (API 1 lists them; the shell honours them as each bundled extension moves over) |
| `approvals.notify`, `releases.read` | Send-approval events; the newest Colecto release |
| `ai.text`, `ai.decide`, `ai.configure` | The owner's AI models and settings |

`app.navigate` is reserved for API keys. Settings and an extension's own values need no
capability.

## Credentials

The shell mints one credential per running client part with the owner bearer:
`POST /api/v1/extensions/{id}/credential` returns `{token}` (32 random bytes, hex). The
Host keeps only a SHA-256 verifier, in memory, bound to the extension and its current
generation; the shell keeps the token in its runtime object and never puts it in
storage, the DOM, a URL, the frame or the Worker.

The shell forwards a client part's API calls with `Authorization: Colecto-Extension
<token>` and `x-colecto-account`, and without the owner bearer. That principal reaches
only these routes; each still checks the extension's capabilities:

| Route | Notes |
| --- | --- |
| `/api/v1/account`, `/messages`, `/messages/*`, `/folders`, `/drafts`, `/drafts/*`, `/labels`, `/send` | Mail, as for API keys |
| `/api/v1/bus/*` | The [bus](#extension-bus), speaking as this extension |
| `/api/v1/extensions/{own id}/settings` | Its own settings only |

Everything else, including accounts, credentials, preferences, policy, API keys, send
decisions and the credential route itself, answers 403. An unknown or revoked token
answers 401.

| Event | Effect on credentials |
| --- | --- |
| Disable, remove or reinstall | The generation changes; every token of that extension fails at its next call |
| Host restart | All tokens are gone; the shell mints new ones |
| Window closes | `DELETE /api/v1/extensions/{id}/credential` with `{token}` (best effort) |
| A ninth token for one extension | The oldest is dropped |

When the local Host forwards to a daemon (`upstream.json`), the credential route goes
to the daemon like any owner route, and calls carrying `Colecto-Extension` pass through
unchanged, so the daemon that holds the policy issues and checks them.

## Client runtime

For each enabled extension with a client part, the shell:

1. Mints a credential and fetches `client/` (owner only:
   `GET /api/v1/extensions/{id}/client` lists the paths, `…/client/{path}` returns one as
   `application/octet-stream`).
2. Creates a hidden `<iframe sandbox="allow-scripts">` (no `allow-same-origin`, so its
   origin is opaque) for `/extension-runtime.html`. The Host serves that page from
   `ui/src/runtime/frame.html` with
   `default-src 'none'; script-src 'sha256-<its script>' blob: 'wasm-unsafe-eval'; worker-src blob:; connect-src 'none'; …; frame-ancestors 'self'`.
3. Posts the files and one end of a `MessageChannel` into the frame, which starts a
   dedicated Worker from a blob holding Colecto's prelude and the entry, and hands it
   the port. The frame relays nothing after that.

| Inside the Worker | Status |
| --- | --- |
| DOM | None |
| `fetch`, `XMLHttpRequest`, `WebSocket`, `EventSource` | Blocked: the blob Worker inherits `connect-src 'none'` |
| App storage, cookies, IndexedDB | Unreachable: the origin is opaque |
| Owner token, its own credential | Never sent |
| Wasm, nested blob Workers | Allowed |

These are checked by `ui/tests/extension-runtime.spec.ts` in Chromium and WebKit.

The prelude defines one global, `colecto`: `colecto.onmessage = (message) => {}`
receives shell messages, `colecto.send(message, transfer?)` sends one, and
`colecto.file(path)` resolves to an `ArrayBuffer` of a file under `client/`. A `.wasm`
entry instead uses the [Wasm client ABI](ui-model.md#wasm-client-abi). Messages are JSON;
transferred buffers are referenced as `{"$buffer": index}`.

| Lifecycle | What happens |
| --- | --- |
| App start, or the extension is enabled or installed | Frame and Worker start; the shell sends `start` |
| Disabled or removed | The Worker and frame are removed and the credential revoked |
| An uncaught error in the Worker, or an invalid or oversized message | It stops; its surfaces show "*name* stopped" with **Restart** |
| No `pong` to two `ping`s (sent every 5 s) | It stops: "*name* stopped responding" |
| Page closes | Credentials are revoked (best effort) |

### Messages

| Shell to extension | Payload |
| --- | --- |
| `start` | `{api: 1, id, theme, account, settings, surfaces}` |
| `account` | `{id, name, address, connected, configured}` when the selected account changes |
| `theme` | [Theme tokens](ui-model.md#theme-tokens) when Appearance changes |
| `settings` | `{settings}` after its settings changed (here or in another window) |
| `event` | `{surface, node, kind, value?}`, for example `kind: "press"` |
| `host` | `{name, data}`: a Host event it may see (see below) |
| `bus.request`, `bus.event` | Bus deliveries to its client part |
| `reply` | `{id, ok, data}` or `{id, ok: false, error, status}` for a `call` |
| `ping` | `{n}`; answer `{t: "pong", n}` |

| Extension to shell | Payload |
| --- | --- |
| `render` | `{surface, tree}` replaces a surface (`null` clears it) |
| `patch` | `{surface, ops}`: `set` (merge props, `null` removes one), `replace`, `insert`, `remove`; all or nothing |
| `call` | `{id, method, path, query?, body?}`: an API call with its credential; at most 64 pending |
| `pong` | `{n}` |

Surfaces today are `pane` (declared by `[pane]`) and `settings` (declared by
`[settings_page]`, shown under the page title with "From *name*"). Other messages in
the [protocol](ui-model.md#protocol), such as toasts, commands and contributions, arrive
with the bundled extensions that first need them.

### Event stream

The shell opens one stream for all running client parts: `POST
/api/v1/extensions/events` with the owner bearer and `{tokens: [...]}` (at most 64).
Each event is `{"to": [extension ids], "data": ...}`; the Host picks recipients by the
same manifest rules as the API, and a revoked token drops out at the next event.

| Host event | Delivered to extensions with |
| --- | --- |
| `mail` | `messages.list` |
| `sync`, `ready`, `resync` | `account.read` |
| `approval` | `approvals.notify` |
| `navigate` | That destination under `navigation` |
| `extension.settings` | Only the extension whose settings changed |
| `bus.request` | The target bus |
| `bus.event` | Its own bus, or subscribers listed under `[bus] subscribe` |

### Components

The renderer (`ui/src/renderer/`) checks every tree against the vocabulary in
`ui/src/runtime/protocol.ts` and draws it with the app's own components and styles.
Unknown types, props or values reject the message. Strings are always text.

| Available now | Components |
| --- | --- |
| Containers | `stack`, `section`, `group`, `toolbar`, `form` |
| Text and media | `heading`, `text`, `badge`, `banner`, `icon` |
| Controls | `button` (`press`), `input`, `textarea` (`change`, or `input` after `debounce_ms`), `toggle` (`toggle`), `select` (`change`); a `form` sends `submit` with `{values}` |
| Collections and states | `list` (static rows), `empty_state`, `loading` |
| Embeds | `frame` without scripts: `title`, `doc` (at most 8 MiB), `height`, `designed` |

Props are listed in the [design](ui-model.md#component-vocabulary); the rest of the
vocabulary arrives with the bundled extension that needs it. Text fields keep their
value in the renderer; a `set` of `value` replaces it.

A `frame` renders `doc` in `<iframe sandbox="allow-popups allow-popups-to-escape-sandbox">`
(no scripts, forms or same-origin access) with
`default-src 'none'; style-src 'unsafe-inline'; img-src data:` and `<base target="_blank">`.

### Icons

`icon` props take one of: `account`, `alert`, `attach`, `back`, `bell`, `check`, `close`,
`compose`, `copy`, `cpu`, `download`, `drafts`, `extension`, `forward`, `image`, `inbox`,
`info`, `key`, `layout`, `mail`, `next`, `palette`, `pencil`, `plus`, `refresh`, `reply`,
`replyAll`, `search`, `sent`, `server`, `settings`, `shield`, `sidebar`, `sort`, `text`,
`trash`. A test keeps this list equal in `manifest.rs`, `protocol.ts` and `Icon.svelte`.

### Limits

| Limit | Value |
| --- | --- |
| Message from a Worker | 4 MiB of JSON, plus 32 MiB of transferred buffers |
| Nodes per surface | 5,000, depth 32 |
| Patch ops per message | 2,000 |
| Text | 16 KiB; labels, titles and options 128 bytes |
| Pending calls | 64 |
| Frame document | 8 MiB |
| Watchdog | Ping every 5 s; stop after two unanswered |

Browsers cannot cap a Worker's CPU or memory. A client part can slow down or crash the
window; see [what this does not protect against](ui-model.md#what-this-does-not-protect-against).

## Service part

The optional `service` module runs in the Host whether or not a window is open. It is
pure Wasm run by `wasmi` without WASI. It exports `memory` and `run`, and may import
only these, from module `colecto`:

| Import | Does |
| --- | --- |
| `action(output, capacity) -> i32` | Copies the invocation's action JSON into memory; bytes written or `-1` |
| `request(input, len, output, capacity) -> i32` | One service request, such as `{"operation":"messages.read","id":"1"}`; writes `{"ok":true,"data":…}` or `{"ok":false,"error":"…"}` |
| `emit(input, len) -> i32` | Its answer to a bus request: `{"bus":{"response":…}}`; `0` or `-1` |

| Action | When |
| --- | --- |
| `{"event":"bus.request","from","name","payload"}` | A request on its own bus |
| `{"event":"bus.event","bus","name","payload"}` | An event it subscribes to |

Service requests use the capability names as operations, plus `settings.read`,
`bus.request` and `bus.emit`. Identity and account come from the Host invocation, never
from guest input. Each invocation gets a fresh instance: 32 MiB memory, 1,024 table
elements, 10,000,000 fuel, eight service requests, 64 KiB per bridge input or output,
32 KiB of output, two invocations at once. Disable, removal and reinstall invalidate
running invocations at their next request. This is an in-process Wasm sandbox, not OS
isolation, and not a security audit; native mail work it asks for still takes its
normal time. Rust service parts can use the [Rust SDK](rust-sdk.md).

## Extension settings

Manifests can declare up to 16 `[[settings]]` tables. Unknown fields, wrong types and
invalid defaults reject the manifest.

| Field | Limit or meaning |
| --- | --- |
| `key` | 1–32 lowercase ASCII letters, digits or underscores; unique |
| `label` | 1–64 UTF-8 bytes |
| `description` | Optional, at most 256 UTF-8 bytes |
| `type` | `toggle`, `range`, `choice`, `text` |
| `default` | Boolean, finite number, option string, or string respectively |
| `min`, `max` | Required finite inclusive bounds for range only |
| `step` | Optional positive finite range increment (control hint) |
| `unit` | Optional range suffix, at most 8 UTF-8 bytes |
| `options` | Choice only: 2–8 distinct string `value` and nonempty `label` pairs |

`GET /api/v1/extensions/{id}/settings` returns `{schema, values}`; `PUT` takes a partial
`{key: value}` object, and unknown keys or invalid values reject the whole update. The
owner and the extension itself (with its credential) may use them. Values persist per
extension, merge over defaults, are deleted on removal, and each change emits
`extension.settings` with `{id}`; the client part then receives `settings`. Service
parts read them with `{"operation":"settings.read"}`.

Without `[settings_page]`, an extension with settings gets a generated page in Settings:
bundled ones in the Built-in group, installed ones under Extensions.

## Extension bus

`extensions.bus` lets enabled extensions talk by name. Each extension owns the bus
matching its ID; disabled extensions have no bus. There is no discovery of buses,
subscribers or names.

| Call | Client part (`call` through the shell) | Service part (`colecto.request`) |
| --- | --- | --- |
| Ask another extension | `POST /bus/{bus}/request` `{name, payload}` → `{data}` | `{"operation":"bus.request","bus","name","payload"}` |
| Publish on its own bus | `POST /bus/events` `{name, payload}` → `{}` | `{"operation":"bus.emit","name","payload"}` |
| Answer a request | `POST /bus/replies/{id}` `{data}` after a `bus.request` message | `emit` `{"bus":{"response":…}}` |

A request to an extension with a service part runs that service part; otherwise it goes
to the extension's client part (or, for bundled extensions not yet on the runtime, its
Svelte code) through the event stream and waits up to 2 seconds. Only the target may
reply, and the first reply wins. Missing buses, failures and unanswered requests all
return null. Events reach service parts directly and client parts through the event
stream, if they subscribe.

| Limit | Value |
| --- | --- |
| Subscriptions per manifest | 32 |
| Request or event name | 1–64 bytes: lowercase ASCII, digits, `-`, `_`, `.`, `/` |
| Payload | 32 KiB serialized JSON |
| Nested service requests | 4; deeper requests return null |
| Client reply wait | 2 seconds; then null |
| HTTP event delivery wait | All service subscribers finish or 1.5 seconds |

Bundled extensions start every name with a whole-number version, `v1/add_button`, never
`v1.5/add_button` or a bare `add_button`; third-party extensions are strongly
encouraged to do the same. The bundled surfaces are listed in
[Bundled bus surfaces](bundled-bus.md).

### Reader slots

Until the reader moves to the runtime, it offers two bundled-only slots that the shell
fills with the `ai` extension's Svelte components: `actions` (toolbar, after Reply /
Reply all / Forward) and `aboveBody` (above the message body). They are props on
`ui/src/extensions/reader/Reader.svelte`, each a snippet receiving a `ReaderSlot`
(`ui/src/lib/reader-slots.ts`): `{id, message, content, reply?(body)}`. The
[contribution slots](ui-model.md#contribution-slots) replace them.

## Workspace panes

Bundled `inbox` and `reader` and every enabled extension with `[pane]` get a pane in the
workspace, next to the core Accounts pane (the account switcher), which is shell UI, not
an extension: hide it from its tab's menu or Settings → Accounts → **Accounts pane**.
Settings → Extensions turns an extension on or off and, for those with a pane, chooses
whether it shows. A newly installed pane joins the reader's group; drag its tab anywhere.
The layout and hidden panes are owner preferences stored by the Host. Panes are never
told where they are placed, so they must work at any size.

## Assets

`GET /api/v1/extensions/{id}/assets/{name}` (owner only, enabled extensions only) returns
an image from `assets/` with its image type, `x-content-type-options: nosniff` and
`content-security-policy: sandbox; default-src 'none'`.

## Local labels

`GET /api/v1/labels` requires `messages.list`; `POST /api/v1/labels` requires
`messages.label`, with `{ "name": "Work", "message_id": null, "remove": false }`: omit
the message ID to create a label, give one to assign it, set `remove` to unassign or
delete. Service parts use `{"operation":"messages.label","label":{...}}`.

Labels persist in `colecto.db`, per account. Names are trimmed, case-sensitive, 1–80
UTF-8 bytes, without control characters; each account allows 50 labels and 1000 message
IDs per label. Labels are local, unencrypted, and do not sync with providers. Drafts do
not take labels.

## HTTP principals

| Principal | Authorization | Reaches |
| --- | --- | --- |
| Owner | `Bearer <owner token>` | Everything; bundled Svelte names itself in `x-colecto-extension` and the account in `x-colecto-account` (routing labels, not authentication) |
| API key | `Bearer colecto_…` | Mail routes and MCP, within its account and capability grants; never Host controls or direct sending |
| Extension | `Colecto-Extension <token>` | See [Credentials](#credentials) |

Cookies never establish authority. See
[daemon authentication](../self-hosting.md#owner-bearers-and-scoped-api-keys).

### Cached folders and error categories

`POST /api/v1/messages/refresh?folder=inbox|sent&q=...` queries the provider; an empty
search updates that folder's bounded snapshot. `GET /api/v1/messages/cache?folder=inbox|sent`
reads without provider access and returns `{folder, messages, fetched_at}`. Cached
searches are rejected. Failed refreshes keep earlier snapshots; Disconnect and Lock clear
active ones. For API keys, message bodies need `messages.read` and draft bodies
`drafts.read`. Errors carry a stable `category` and a readable `error`;
`delivery_uncertain` means check Sent before sending again.
