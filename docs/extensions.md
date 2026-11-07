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

Run `just extension-example` to compile the example's readable WAT source into
`target/extensions/connection-panel.wasm`. Start `just demo`, open **Extensions**,
and select `extensions/examples/connection-panel/extension.toml` and that Wasm
binary. Installation leaves the package disabled with no account grants. Enable
it, select the account, and explicitly grant `account.read`. Its **Connection
panel** entry appears in mail navigation. Without that grant, the example shows a
permission message; with the grant, it shows connection state. Revoke access or
remove the extension through Host controls. Saved mail is never removed.

The runtime accepts binary Wasm, not JavaScript, Svelte, HTML, arbitrary URLs or
WASI applications. Third-party UI currently contributes **panels**; replacing the
bundled mailbox, reader, composer or shell with a Wasm package is not supported.
Those bundled components remain explicitly registered trusted Svelte code, with
TOML manifests in `extensions/<id>/extension.toml`. They initially receive their
manifest permissions. Account and credential protection, extension consent and
failure recovery remain trusted Host controls.

## Capabilities and persistence

Capabilities are `account.read` (connection state only), `messages.list`,
`messages.read`, `drafts.list`, `drafts.read`, `drafts.write`, and `messages.send`.
A package can only receive capabilities declared in its manifest. Grants are
scoped to the selected account; enabled state applies globally. Third-party
packages have no grants for new accounts. Enabling a package does not grant
permissions. Granting `messages.send` allows the package to send mail, so grant
it only to packages you trust with delivery.

The Host binds extension identity and account to each invocation. Guest input
cannot override them. Every service call rechecks current grants and the invocation generation.
Installing, removing or disabling a package invalidates existing invocations
on their next service call, so an old instance cannot borrow a replacement
package's grants even when its ID is reused. An operation
already authorized may finish after revocation; later calls are rejected.
`messages.list` returns the account's cached messages rather than triggering
network synchronization. Account creation, credential access, vault controls and
policy changes are not guest operations.

Packages and grants live in a versioned `extensions.json` beside the account
stores. Writes use a temporary file, file synchronization and atomic rename;
writes failing before rename do not alter in-memory permissions. Unix builds
also synchronize the parent directory after rename. A subsequent durability
error is reported while retaining the already committed state; controls reload
authoritative permissions even on errors. Other platforms require separate
durability testing. A corrupt, invalid or newer
policy stops startup instead of silently resetting revoked access. Do not delete
this file as a routine repair: doing so resets bundled grants and removes
installed packages. Keep a backup before manual recovery. No mail, credential or
account storage formats change. Older binaries ignore this file and lack its
revocation enforcement; downgrading is not a security-preserving rollback.
Reinstalling requires removal first and starts again without grants.

## Wasm ABI

Export a linear memory named `memory` and a function `run: () -> ()`. Each action
creates a fresh instance: no background execution or retained guest state. The
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
credential, database or native IPC imports. Limits per invocation: 1,000,000
fuel, one memory of at most 2 MiB, one table of at most 1,024 elements, eight
service requests, 32 KiB per bridge input/output, 16 KiB view text, 256-byte
title, sixteen actions of at most 128 bytes each. Traps and exhausted limits
return an extension error without removing Host controls or saved mail. At most
two invocations execute concurrently; each package is at most 64 KiB, each
manifest at most 8 KiB, and at most sixteen third-party packages are installed.

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
then run `just native`. `just native-check` compiles the optional adapter. `just native-test` builds
and smoke-tests the Linux/X11 window and loopback registry under Xvfb (requires
`xvfb` and `xwininfo`). The
Tauri window loads the same loopback Host and confines navigation to that origin.
It registers no custom Tauri commands, plugins or capability grants. Wasm guests
execute server-side and cannot access a webview or native IPC. Tauri's native
[capabilities](https://v2.tauri.app/security/capabilities/) are distinct from the
account-scoped service capabilities above; neither is used as a substitute for
Wasm isolation. Mobile builds and other desktop platforms require separate
platform testing; this runner does not claim mobile parity.
