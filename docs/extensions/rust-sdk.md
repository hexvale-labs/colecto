# Rust extension SDK

`colecto-extension` lets you write an extension's [service part](README.md#service-part)
as plain Rust functions: typed service calls, typed bus messages with versioned names
checked at compile time, and a mock Host for `cargo test`. The service part runs in the
Host without a window. The client part, which draws the interface, is JavaScript or Wasm
in `client/` (see the [guide](guide.md#3-write-the-client-part)); `cargo colecto`
packages both. Rust support for client parts (the [Wasm client
ABI](ui-model.md#wasm-client-abi)) is not in the SDK yet.

## Crates

| Crate | Path | Role |
| --- | --- | --- |
| `colecto-extension` | `crates/colecto-extension/` | The SDK you depend on. Forbids `unsafe` code. |
| `colecto-extension-sys` | `crates/colecto-extension-sys/` | The three `colecto` service imports, the `run` export, handler records and the native mock. The only SDK crate that contains `unsafe` code. |
| `cargo-colecto` | `crates/cargo-colecto/` | The `cargo colecto` build and packaging tool. |

Extensions themselves need no `unsafe` code; build with `#![forbid(unsafe_code)]`. The
SDK uses `std` and `serde_json`, so a release build is a little over 100 KB of Wasm: well
within the 16 MiB module limit, but bigger than a hand-written `no_std` module.

## Use it from your own project

The SDK is not on crates.io. Depend on it through Git, pinned to a release tag; Cargo
finds the crate by name inside the repository's workspace. The SDK and
`cargo-colecto` ship from v0.3.0 on; use `branch = "develop"` instead of `tag` for
unreleased changes.

```toml
[package]
name = "my-extension"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
colecto-extension = { git = "https://github.com/hexvale-labs/colecto", tag = "v0.3.0" }
serde = { version = "1", features = ["derive"] }

[profile.release]
opt-level = "z"
lto = true
panic = "abort"
strip = true
codegen-units = 1
```

| Need | Do |
| --- | --- |
| Pin a version | Use `tag` (or `rev`). `Cargo.lock` records the commit; `cargo update -p colecto-extension` moves it. |
| Work against a local checkout | Add `[patch."https://github.com/hexvale-labs/colecto"]` with `colecto-extension = { path = "../colecto/crates/colecto-extension" }`. |
| Build and package | `rustup target add wasm32-unknown-unknown`, then `cargo colecto build` (see [below](#build-and-package-with-cargo-colecto)). |
| Test | `cargo test` on your own machine. The imports exist only on `wasm32`, so native builds use the mock Host. |

The SDK depends only on `serde` and `serde_json`, so a Git dependency does not build
the mail service.

## Build and package with `cargo colecto`

`cargo-colecto` is a Cargo subcommand that builds the service part for
`wasm32-unknown-unknown`, checks it against the manifest and writes the `.colectoext`
with the client part. Install it from the same tag as the SDK:

```sh
cargo install --git https://github.com/hexvale-labs/colecto --tag v0.3.0 cargo-colecto
```

Inside this repository, `cargo run -p cargo-colecto -- <command>` runs it without
installing.

| Command | Does |
| --- | --- |
| `cargo colecto build` | Builds in release mode, checks, writes `<target-dir>/colecto/<id>.colectoext` |
| `cargo colecto check` | Builds and checks without writing a package |

| Option | Meaning |
| --- | --- |
| `--manifest-path <PATH>` | The extension's `Cargo.toml` (default: the package in the current directory) |
| `--out <PATH>` | Package path for `build` |
| `--locked` | Passed on to `cargo build` |

The project layout it expects, next to `Cargo.toml`: `extension.toml` (the manifest,
which must name the built module with `service = "service.wasm"` and the client entry with
`client = "client/main.js"`), the `client/` folder, an optional `assets/` folder of
images, and a library with `crate-type = ["cdylib"]`. The package is Deflate-compressed and read back with the
Host's own package reader (`colecto-package`), so every package and manifest limit is
checked before you install.

`export!` records each registered function in a `colecto.handlers.v1` custom section
of the Wasm module: the bus and name of each request and event. The section survives `lto` and `strip`, and costs a few bytes. The checks:

| Check | Why |
| --- | --- |
| Imports are only `colecto.action`, `colecto.request`, `colecto.emit`; `run` and `memory` are exported | The Host refuses anything else, including WASI |
| `extension.toml` declares `service` | The built crate is the service part |
| Bus handlers need `extensions.bus` in `capabilities` | Without it the Host delivers nothing |
| Every `events` entry is in `[bus] subscribe`, and every subscription has a function | A missing subscription means the event never arrives; an extra one wakes the extension for nothing. The error suggests the exact `[bus]` table. |
| Each request is declared on this extension's own bus (`id`) | Requests only arrive on the extension's own bus |

Requests registered twice are warnings. A module built
without `export!` gets a warning and only the import, export and package checks. The
tool cannot see calls such as `cx.messages()`, so it does not check the other
capabilities.

## Write the module

Register your functions with `export!`. It defines the `run` export, reads the action,
calls the matching function and emits its result. The message type for each function
comes from its first parameter.

```rust
#![forbid(unsafe_code)]
use colecto_extension::prelude::*;

#[derive(Serialize, Deserialize)]
struct Count {}
colecto_extension::bus_request!(Count => usize, bus = "counter", version = 1, name = "count");

fn count(_: Count, cx: &mut Context) -> Result<usize> {
    Ok(cx.messages()?.len())
}

colecto_extension::export! { requests: [count] }
```

| Entry | Function signature | Runs for |
| --- | --- | --- |
| `requests: [f, …]` | `fn(R, &mut Context) -> Result<R::Response>` where `R: BusRequest` | Bus requests to your bus named `R::NAME` |
| `events: [f, …]` | `fn(E, &mut Context) -> Result<()>` where `E: BusEvent` | Events from bus `E::BUS` named `E::NAME` |
| `mail: f` | `fn(MailEvent, &mut Context) -> Result<()>` | New mail: `MailEvent {account, ids}`; needs `messages.list`. The bundled AI auto-labeller (`ui/src/extensions/ai/service/`) is an example |

Entries may come in any order. When nothing matches, nothing is emitted, which the Host treats as no answer.

What happens on failure:

| Situation | Result |
| --- | --- |
| Request payload does not parse, or the function returns `Err` | No answer; the caller gets null |
| Event payload does not parse, or the function returns `Err` | Ignored (the Host ignores event output) |
| Panic | The guest traps; the Host reports an extension error |

## Call the Host

`Context` wraps the service operations. Each call needs the capability in parentheses,
acts on the account the owner has selected, and counts toward the limit of 64 calls
per invocation. Errors the Host reports come back as `Error::Service`, and refused
bridge calls as `Error::Bridge`.

| Method | Operation (capability) | Returns |
| --- | --- | --- |
| `settings::<S>()` | `settings.read` (none) | Your `[[settings]]` values, merged over defaults |
| `account_connected()` | `account.read` | `bool` |
| `messages()` | `messages.list` | `Vec<Message>` |
| `message(id)` | `messages.read` | `Message` |
| `drafts()` | `drafts.list` | `Vec<Draft>` |
| `draft(id)` | `drafts.read` | `Draft` |
| `save_draft(&DraftInput)` | `drafts.write` | `Draft` |
| `send(&DraftInput)` | `messages.send` (plus `messages.read` with `forward_of`) | `()` |
| `label(name, message_id, remove)` | `messages.label` | `Labels` (label name to message IDs) |
| `labels()` | `labels.list` (`messages.list`) | `Labels` |
| `stored::<T>(key)`, `store(key, &value)` | `storage.get`, `storage.set` (none) | `Option<T>`, `()` |
| `request(&R)` | `bus.request` (`extensions.bus`) | `Option<R::Response>` |
| `emit(&E)` | `bus.emit` (`extensions.bus`) | `()` |
| `call::<T>(&request)` | Any raw request | The response `data` as `T` |
| `sender()` | — | The calling extension's ID while answering a bus request |

### Forward compatibility

The SDK keeps its own copies of the wire types (`Message`, `Draft`, bus payloads). Unlike
the Host's types in `colecto-extension-api`, they ignore unknown fields and default
missing ones, so a newer Host can add fields without breaking installed extensions.
A test in `crates/colecto-extension/src/types.rs` checks that the copies read what the
Host writes, and that the Host accepts the `DraftInput` the SDK sends. New fields added
to these copies must have defaults.

## Bus messages

A bus message is a payload type with a bus, a version and a name. Declare one with a
macro; the version becomes the `vN/` prefix:

```rust
#[derive(Serialize, Deserialize)]
struct WordCount { message_id: String }
#[derive(Serialize, Deserialize)]
struct Count { words: usize }

colecto_extension::bus_request!(WordCount => Count, bus = "word-counter", version = 1, name = "word_count");
colecto_extension::bus_event!(Counted, bus = "word-counter", version = 1, name = "counted");
// WordCount::NAME == "v1/word_count"
```

The macros enforce the Host's rules at compile time: the bus ID must use lowercase ASCII
letters, digits and hyphens, the version must be at least 1, and the full name must be
1–64 bytes of lowercase ASCII letters, digits, `-`, `_` and `/`. Use `()` as the response
type for requests answered with `null`.

`Context::request` returns `Ok(None)` when the bus is missing or disabled, nothing
answered, or the answer was `null`; the Host does not say which. Events only arrive
when the manifest lists them under `[bus] subscribe`, so add each `events` entry there.
`Context::emit` always publishes on your own bus.

The bundled surfaces are ready-made in `colecto_extension::bundled`, one module per
version:

| Module | Requests | Events |
| --- | --- | --- |
| `composer::v1` | `GetDraft` → `Draft`, `SetDraft`, `AddButton`, `RemoveButton` | `Opened`, `Sending` |
| `reader::v1` | `GetHtml` → `String`, `SetHtml`, `AddBanner` | `MessageOpened` |
| `inbox::v1` | — | `Selected` |
| `notifications::v1` | `Notify` | — |

A composer button sends your request with `{"draft": composer::v1::Draft}`. Answer with
`{"draft": composer::v1::SetDraft}` to change the editor.

## Test without Wasm

`colecto_extension::testing::TestHost` answers service requests with a closure. `run`
calls the `colecto_main` function that `export!` defines, with any action, and returns
the emitted JSON. `with` hands one handler a `Context` directly.

```rust
#[test]
fn counts_messages() {
    let mut host = TestHost::new(|request| match request["operation"].as_str() {
        Some("messages.list") => Ok(json!([{"id": "1"}])),
        _ => Err("Capability not granted".into()),
    });
    let action = r#"{"event":"bus.request","from":"x","name":"v1/count","payload":{}}"#;
    assert_eq!(host.run(crate::colecto_main, action).unwrap()["bus"]["response"], 1);
}
```

The mock follows the Host's bridge rules (only the first emit counts), but it does not
enforce fuel, memory, call or size limits; `crates/colecto-host/src/sandbox.rs` tests
those in the real sandbox.
