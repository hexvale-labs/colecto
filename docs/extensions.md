# Extension contract (API 1)

Every package has an `extension.toml` manifest:

```toml
id = "connection-panel"
name = "Connection panel"
description = "Check the selected account connection."
author = "Your name"
api_version = 1
slot = "panel"
capabilities = ["account.read"]
```

All fields above are required. Unknown fields, unknown or duplicate capabilities,
invalid identifiers, empty metadata, unsupported slots and API versions are
rejected. Identifiers use lowercase ASCII letters, digits and hyphens. Author
names are self-reported metadata, not verified publisher identities.

## Install and run a third-party extension

Third-party extensions ship as one `.colectoext` file: a zip archive holding
`manifest.toml` (the manifest above, at most 8 KiB) and `module.wasm` (at most 16 MiB),
with Stored or Deflate entries and at most 16 MiB in all. Up to 256 optional raster image assets are allowed under `assets/`. Anything else, including unsafe
paths, or entries that decompress past their limit, is rejected before the
manifest is parsed. `colecto_host::package::write` builds one.

Run `just build extension` to compile the example's readable WAT source and package it
as `target/extensions/connection-panel.colectoext`. Start `just run`, open **Settings →
Extensions**, pick that package and press **Install**. A warning explains that the
permission system cannot stop an extension from lying about its purpose or misusing data
it is allowed to read, and recommends open-source extensions from known authors; its
Install button unlocks after five seconds. Installation enables the extension with every requested capability on all accounts.
Disable or remove it through Settings → Extensions. Saved mail is never removed.

The runtime accepts binary Wasm, not JavaScript, Svelte, HTML, arbitrary URLs or
WASI applications. Third-party UI currently contributes **panels and overlays**; replacing the
bundled mailbox, reader, composer or shell with a Wasm package is not supported.
Those bundled components remain explicitly registered trusted Svelte code, with
TOML manifests in `ui/src/extensions/<id>/extension.toml`. They initially receive their
manifest permissions. Account and credential protection, extension consent and
failure recovery remain trusted Host controls.

## Capabilities and persistence

Enabled extensions have every capability declared by their manifest on every account.
Disabled extensions have none. Capabilities cannot be granted individually. The Host
checks enabled state, manifest declarations and invocation generation at each service
call. Bundled extensions obey the same rule. Obsolete stored account grants are ignored.
Installing enables the extension; the five-second warning lists all capabilities in
plain words and is the only consent step. Cancel installs nothing.

Policy and installed packages persist in the Host database. Disable and remove invalidate
existing invocations at their next service call. Already authorized operations may finish.
Account creation, credentials and policy changes remain Host-only operations.


## Wasm ABI

Export a linear memory named `memory` and a function `run: () -> ()`. Each action
creates a fresh instance from the cached compiled module: no retained guest state. The
Host passes `open` on initial display and a clicked action's exact text on later
invocations. Modules may import only these functions from module `colecto`:

- `action(output: i32, capacity: i32) -> i32`: copy the current action's UTF-8
  bytes into guest memory; return bytes written or `-1`.
- `request(input: i32, length: i32, output: i32, capacity: i32) -> i32`: read a
  UTF-8 JSON service request and write a JSON response; return bytes written or
  `-1`. Example input: `{"operation":"messages.read","id":"1"}`. Draft write
  and send use `{"operation":"drafts.write","draft":{"recipient":"...",
  "subject":"...","body":"..."}}`. Unknown request fields are rejected.
  Responses are `{"ok":true,"data":...}` or `{"ok":false,"error":"..."}`.
- `emit(input: i32, length: i32) -> i32`: emit one UTF-8 JSON view of shape
  `{"title":"...","text":"...","actions":["Refresh"]}`. Return `0` on success
  or `-1` on invalid output. A completed invocation must emit a valid view.

Only text and buttons are rendered, through Svelte's escaping. Strings never
become HTML, script, CSS, URLs or Tauri commands. Actions are untrusted labels;
the Host owns actual buttons and invocation. See the WAT example for a complete
module, or compile your language to this minimal ABI without WASI.

## Resource and trust boundaries

Wasmi 2 runs guests without WASI or filesystem, networking, process, clock,
credential, database or native IPC imports. Limits per invocation: 10,000,000
fuel, one memory of at most 32 MiB, one table of at most 1,024 elements, eight
service requests, 32 KiB per bridge input/output, 16 KiB view text, 256-byte
title, sixteen actions of at most 128 bytes each. Traps and exhausted limits
return an extension error without removing Host controls or saved mail. At most
two invocations execute concurrently; each module is at most 16 MiB, each
manifest at most 8 KiB, and at most 32 third-party packages are installed.

This is an in-process Wasm language/runtime sandbox, not OS process isolation or
a claim of a security audit. Keep runtime security updates current. Native mail
operations authorized by a guest still run in Core services and can take their
normal transport timeout; fuel does not meter native service work. Pure guest
CPU and memory are bounded. The current binary is a local single-user prototype,
not a remotely authenticated multi-user service.

Trusted bundled code shares the application origin. Header identities on its
HTTP path are routing labels, not authentication against compromised same-origin
scripts. Wasm guests never receive that path, browser execution or headers; the
Host rejects third-party IDs on direct mail HTTP routes. Do not import untrusted
JavaScript into the app origin.

## Tauri desktop

Install the official [platform prerequisites](https://v2.tauri.app/start/prerequisites/),
then run `just run native`. `just check native` compiles the optional adapter. `just test native` builds
and smoke-tests the Linux/X11 window and loopback registry under Xvfb (requires
`xvfb` and `xwininfo`). The
Tauri window loads the same loopback Host and confines navigation to that origin.
It registers no custom Tauri commands, plugins or capability grants. Wasm guests
execute server-side and cannot access a webview or native IPC. Tauri's native
[capabilities](https://v2.tauri.app/security/capabilities/) are distinct from the
extension service capabilities above; neither is used as a substitute for
Wasm isolation. Mobile builds and other desktop platforms require separate
platform testing; this runner does not claim mobile parity.


## Workspace panes

Bundled `inbox`, `reader` and `accounts` extensions and every enabled third-party
`panel` each get a pane in the workspace. Settings → Extensions turns an extension on
or off and, for those with a pane, chooses whether it shows in the workspace. A newly
installed panel joins the reader's group once enabled; drag its tab anywhere from
there. The layout and hidden panes are owner preferences stored by the Host. Panes
are never told where they are placed, so they must work at any size.

## Local labels

`GET /api/v1/labels` requires `messages.list`; `POST /api/v1/labels` requires
`messages.label`. The writer accepts `{ "name": "Work", "message_id": null,
"remove": false }`: omit the message ID to create a label, provide a currently
known message ID to assign it, and set `remove` to unassign or delete. Guest
service requests use `{"operation":"messages.label","label":{...}}` with the same
input. All operations bind to the invocation account and require an enabled extension
that declares the capability.

Labels persist in a versioned `labels.json` next to each account's mail store.
Names are trimmed, case-sensitive, 1–80 UTF-8 bytes, without control characters;
each account permits 50 labels and 1000 message IDs per label, with a 2 MB file
limit. Writes are atomic; invalid or unreadable stores are never silently reset.
Labels are local service metadata, unencrypted, and do not sync with providers.
Filtering applies to the current bounded Inbox/Sent search result, not full
provider history. Drafts are searchable locally but do not accept message labels.

## HTTP principal authentication

Bundled UI account/module headers route operations only after owner bearer
authentication. Every local and remote owner request explicitly supplies
`Authorization: Bearer <owner-token>`; cookies never establish owner authority.
Local mode uses a private OS-user-owned `owner-token` file; loopback headers alone
cannot establish ownership. Remote mode uses the configured owner bearer. The
native shell injects the local token only at its exact local origin, never into
a remote server page. The browser keeps the bearer in origin-scoped sessionStorage
and does not automatically send it to another port. `POST /api/v1/session` only
validates an owner bearer with exact Host/Origin; it creates no cookie or session.
API keys use explicit account/capability grants independent of extension headers
and cannot invoke Host controls or directly send mail. Wasm service calls retain
their Host-bound enabled state and manifest capabilities.
See [daemon authentication](daemon.md#owner-bearers-and-scoped-api-keys).

### Cached folders and error categories

`GET /api/v1/messages` retains its provider-query behavior.
`POST /api/v1/messages/refresh?folder=inbox|sent&q=...` explicitly queries the
provider; an empty search updates that folder's bounded snapshot.
`GET /api/v1/messages/cache?folder=inbox|sent` reads without provider access and
returns `{folder, messages, fetched_at}`; `fetched_at` is Unix seconds or null
for unknown freshness. Cached searches are rejected rather than mistaken for
complete provider search results. Failed refreshes preserve prior snapshots.
Network outages keep cached readers available. Explicit owner Disconnect and
vault Lock clear active snapshots; on-disk snapshots remain for identity-checked
reconnect. They do not make cached messages accessible while locked/disconnected.
MCP exposes `messages.cached` and `messages.refresh` alongside compatible
`messages.list`. All require `messages.list`.

For API keys, message-list bodies require the same account's `messages.read`;
draft-list bodies require `drafts.read`. Without the read grant, bodies are empty
strings and metadata remains available. Owner, bundled, and existing Wasm grant
semantics remain unchanged. HTTP and MCP errors include a stable `category` plus
a readable `error`. Known provider status codes are safe bounded details; raw
server descriptions, URLs and credentials are never copied into errors.
`delivery_uncertain` means check Sent and request status before sending again.

## Overlay extensions

Declare `window.overlay`, `slot = "overlay"` and `z_index = 10` (a good default).
`z_index` is required with this capability, forbidden without it, and must be an
integer from 1 through 10000. Bundled overlays use 500. Settings uses 1000;
menus use 1100, the command runner 1200, and the install dialog 1300. Native
modal dialogs also use the browser top layer. Oneko uses 10000 intentionally.

The Host renders each enabled overlay in a fixed viewport container with
`pointer-events: none`. Guests remain fresh Wasmi instances with the same fuel,
memory and service-call limits as panels. While visible, the UI calls the existing
run route approximately every 100 ms, with at most one call in flight per overlay.
Errors stop the loop and appear as extension errors. State round-trips explicitly.

The action is JSON (at most 32768 UTF-8 bytes):

```json
{"event":"frame","time_ms":123,"pointer":{"x":100,"y":200,"inside":true},"viewport":{"width":1280,"height":720},"state":""}
```

Emit through the existing `colecto.emit` import:

```json
{"overlay":{"items":[{"image":"cat.png","x":30,"y":40,"width":32,"height":32,"frame":{"x":0,"y":0,"width":32,"height":32}}]},"state":"next"}
```

Unknown output fields are rejected at every level. At most 16 items are allowed.
All coordinates must be finite; frame origins must be nonnegative. Every width and
height must be positive and at most 512. State is at most 4096 UTF-8 bytes. Images
must name assets from the emitting package. `frame` crops the sprite sheet; output
width and height scale that crop. Panels keep their text view ABI unchanged.

Assets use flat safe names `[a-z0-9-]+\.(png|jpg|jpeg|webp|gif)`, at most 8 MiB each,
with magic bytes verified against the suffix. SVG and nested paths are rejected. Packages accept at most 256 assets, 16 MiB compressed total, 8 KiB
manifest and 16 MiB module. No extra paths, duplicate entries or other compression
methods are accepted. `GET /api/v1/extensions/{id}/assets/{name}` requires owner
route authentication and Host identity, and refuses disabled extensions. It returns
the matching image content type with `x-content-type-options: nosniff` and
`content-security-policy: sandbox; default-src 'none'`. The UI downloads with auth headers
and creates local image blob URLs; scripts cannot use blob URLs.
`POST /api/v1/extensions/review` validates a package and returns its manifest without
installing it, allowing the consent dialog to list capabilities.

See [Make your own extension](extensions/guide.md) for a walkthrough and API policy.

## Large packages and compiled modules

Limits are defined once in `crates/colecto-host/src/package.rs`: compressed archive
16 MiB; total uncompressed content 48 MiB; module 16 MiB; manifest 8 KiB; up to
256 flat raster assets of 8 MiB each; at most 32 installed third-party packages.
The Host checks every declared entry size and total before inflation, caps actual
inflation by per-entry and aggregate budgets, and rejects any entry above 100:1
compression. Raster magic must match PNG, JPEG, WebP or GIF suffixes; SVG is forbidden.

The Host compiles enabled modules once on installation, startup or re-enabling.
The cache associates each package ID with its generation. Every invocation creates
a fresh instance with 32 MiB memory, 10,000,000 fuel, and the existing two-invocation
concurrency cap. Disable and removal drop the compiled entry; reinstall gets a new
generation. Policy contains metadata only; compressed package bytes live in separate
database records. Listing extensions never reads module bodies or recompiles them.
The UI uploads binary bodies; legacy JSON byte-array uploads are still accepted.
