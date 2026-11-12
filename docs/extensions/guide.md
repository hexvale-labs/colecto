# Make your own extension

## 1. Choose capabilities

Start with the smallest useful operation. `account.read` reads connection status;
`messages.list` and `messages.read` expose mail; `drafts.list`, `drafts.read` and
`drafts.write` handle drafts; `messages.send` delivers mail; `messages.label`
manages labels. `window.overlay` draws sprites over the app.

Users consent once during installation. Enabled means all declared capabilities
on every account. Disabled means none. There are no individual permission grants.

## 2. Write a manifest

Create `extension.toml` next to your source:

```toml
id = "my-panel"
name = "My panel"
author = "Your name"
description = "Show the account connection status."
api_version = 1
slot = "panel"
capabilities = ["account.read"]
```

All these fields are required. IDs contain lowercase ASCII letters, digits and
hyphens. Unknown fields, duplicate capabilities and unsupported APIs are rejected.
For an overlay, use `slot = "overlay"`, declare `window.overlay` and add `z_index`.
The integer must be between 1 and 10000 and is forbidden without that capability.

## 3. Write the module

Use the readable WAT in `examples/connection-panel/module.wat` as a panel starting
point. Export `memory` and `run`. Only three imports are available, all in the
`colecto` namespace: `action(ptr, capacity)`, `emit(ptr, len)` and
`request(ptr, len, output, capacity)`. Return values are byte lengths (action and
request), zero for successful emit, or -1 on failure. Strings are UTF-8 JSON.

A panel emits `{"title":"Hello","text":"Plain text","actions":["Refresh"]}`.
The selected action is passed back on the next invocation. Service requests use
an `operation` capability and operation-specific arguments. Identity and account
come from the Host and cannot be supplied by the guest.

For Rust, copy the standalone `examples/oneko` crate. It uses `#![no_std]`, imports
the ABI directly, and exports `run`. Build for `wasm32-unknown-unknown`, without
WASI. Keep the module below 16 MiB. Every invocation starts fresh; overlays store
state in the emitted string and receive it back in the next frame action.
See [the contract](README.md#overlay-extensions) for exact frame JSON and limits.

## 4. Pick an overlay layer

| Surface | Layer |
| --- | ---: |
| Workspace | 0 |
| Third-party overlay, a good default | 10 |
| Bundled extension overlays | 500 |
| Settings card | 1000 |
| Menus and popovers, including Settings controls | 1100 |
| Command runner | 1200 |
| Install dialog | 1300 |
| Oneko, an example of “always on top” | 10000 |

The registry is `ui/src/lib/layers.ts`. Modal dialogs additionally occupy the
browser's top layer. Notifications sit below Settings. Overlay containers never
intercept pointer events.

## 5. Package and build

A `.colectoext` file is a ZIP containing `manifest.toml`, `module.wasm`, and optional
`assets/*.(png|jpg|jpeg|webp|gif)`. Rename your source manifest inside the archive. Use Stored or
Deflate compression. Assets need safe lowercase flat names, verified PNG/JPEG/WebP/GIF magic bytes, at most
8 MiB each and at most 256 files. Total compressed package size is 16 MiB.

`just build extension` packages the WAT example. `just build oneko` installs the
Rust Wasm target if needed, builds the Rust guest and writes
`docs/extensions/examples/oneko.colectoext`. The example is not bundled.
The Host's `package::write` helper builds asset-free packages; the Oneko packaging
script shows how to include sprites. Never include source files or directory entries.

## 6. Install and debug

Open Settings → Extensions and choose your package. Read the warning and capability
list. After five seconds, Install enables it. Cancel leaves the app unchanged.
Settings shows read-only capabilities, an enable switch, workspace visibility for
panels, and Remove for third-party packages.

If installation fails, check the manifest, ZIP entries and size limits first.
If execution fails, check imports, exported memory/run, JSON shape and fuel usage.
Panels have a Retry control. Overlay errors stop frame calls; toggle the extension
off and on to retry. Service errors are returned as `{"ok":false,"error":"…"}`.
Never assume the selected account is connected. Guest text is never interpreted
as HTML. No filesystem, network, credentials, native IPC or retained guest memory
is available. Use the test suites to check trust boundaries before sharing a package.

## API version policy

Every extension targets one `api_version`. During alpha the version stays 1 and
things may break. After release it increases with every major update so old
extensions keep working on the version they target.

If a later change, such as tighter security, means an older API version can no
longer work fully, the app will warn that the extension might not function
correctly. That warning is planned, not implemented.

Legacy notification preferences did not record whether bottom right was chosen or
merely the default. Unmarked bottom-right positions reset to top center once;
other saved positions remain. New position choices are recorded explicitly.

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

### Add owner settings

Add a table to `extension.toml`, then read it through `settings.read` at runtime:

```toml
[[settings]]
key = "speed"
label = "Running speed"
type = "range"
default = 10
min = 2
max = 30
step = 1
unit = "px"
```

The owner edits this in Settings. See Oneko's `src/lib.rs` for a no_std guest
that reads speed, opacity and stopping distance once per frame using one bridge
call. The [contract](README.md#extension-settings) lists validation and routes.

## Talk to another extension

Declare `extensions.bus` and subscribe to `reader/v1/message_opened` under `[bus]`
when you need message-open events. A guest can ask the composer for its current draft:

```json
{"operation":"bus.request","bus":"composer","name":"v1/get_draft","payload":null}
```

Read `data` from the service response; null also means no answer. To answer a request
on your own bus, inspect the `bus.request` action and emit
`{"bus":{"response":{"example":true}}}`. Each delivery starts a fresh instance,
so keep needed context in the payload. Use versioned names such as `v1/example`.
See the [bus contract](README.md#extension-bus) for limits and bundled surfaces.
