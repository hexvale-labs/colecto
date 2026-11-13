# Rust extension SDK

`colecto-extension` lets you write a third-party extension as plain Rust functions:
typed service calls, typed bus messages with versioned names checked at compile time,
and a mock Host for `cargo test`. It compiles to the [Wasm ABI](README.md#wasm-abi);
the manifest, packaging and limits are unchanged.

## Crates

| Crate | Path | Role |
| --- | --- | --- |
| `colecto-extension` | `crates/colecto-extension/` | The SDK you depend on. Forbids `unsafe` code. |
| `colecto-extension-sys` | `crates/colecto-extension-sys/` | The three `colecto` imports, the `run` export and the native mock. The only SDK crate that contains `unsafe` code. |

Extensions themselves need no `unsafe` code: Oneko builds with `#![forbid(unsafe_code)]`.
The SDK uses `std` and `serde_json`. A release build of Oneko is about 140 KB of Wasm.
That is well within the 16 MiB module limit, but bigger than a hand-written `no_std`
guest.

## Use it from your own project

The SDK is not on crates.io. Depend on it through Git, pinned to a release tag; Cargo
finds the crate by name inside the repository's workspace. The SDK first ships in the
release after 0.2.0; until then use `branch = "develop"` instead of `tag`.

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
| Build | `rustup target add wasm32-unknown-unknown`, then `cargo build --release --target wasm32-unknown-unknown`. No WASI. |
| Test | `cargo test` on your own machine. The imports exist only on `wasm32`, so native builds use the mock Host. |

The SDK depends only on `serde` and `serde_json`, so a Git dependency does not build
the mail service. Package the `.wasm` file as described in
[Package and build](guide.md#5-package-and-build).

## Write the module

Register your functions with `export!`. It defines the `run` export, reads the action,
calls the matching function and emits its result. The message type for each function
comes from its first parameter.

```rust
#![forbid(unsafe_code)]
use colecto_extension::prelude::*;

fn panel(event: PanelEvent, cx: &mut Context) -> Result<View> {
    let count = cx.messages()?.len();
    Ok(View::new("Inbox", format!("{count} messages")).action("Refresh"))
}

colecto_extension::export! { panel: panel }
```

| Entry | Function signature | Runs for |
| --- | --- | --- |
| `panel: f` | `fn(PanelEvent, &mut Context) -> Result<View>` | Panel display (`PanelEvent::Open`) and button presses (`PanelEvent::Action(label)`) |
| `overlay: f` | `fn(Frame<S>, &mut Context) -> Result<Overlay<S>>` | Each overlay frame |
| `requests: [f, …]` | `fn(R, &mut Context) -> Result<R::Response>` where `R: BusRequest` | Bus requests to your bus named `R::NAME` |
| `events: [f, …]` | `fn(E, &mut Context) -> Result<()>` where `E: BusEvent` | Events from bus `E::BUS` named `E::NAME` |

Use `panel` or `overlay` to match the manifest `slot`, not both. Entries may come in any
order. When nothing matches, nothing is emitted, which the Host treats as no answer.

What happens on failure:

| Situation | Result |
| --- | --- |
| Panel function returns `Err` | A view titled `Error` with the error text |
| Panel view is over the limits | A view titled `Error` naming the limits |
| Overlay function returns `Err` | Nothing emitted; the Host reports an error and stops the overlay |
| Request payload does not parse, or the function returns `Err` | No answer; the caller gets null |
| Event payload does not parse, or the function returns `Err` | Ignored (the Host ignores event output) |
| Panic | The guest traps; the Host reports an extension error |

### Overlay state

The Host round-trips overlay state as a string. The SDK stores your state type `S` as
JSON in that string. `S` needs `Serialize`, `Deserialize` and `Default`. The first
frame gets `S::default()`, and so does any frame whose stored state no longer parses,
for example after an update changed `S`. The JSON must fit in 4096 bytes. See
`docs/extensions/examples/oneko/src/lib.rs`.

## Call the Host

`Context` wraps the service operations. Each call needs the capability in parentheses,
acts on the account the owner has selected, and counts toward the limit of eight calls
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
fn shows_message_count() {
    let mut host = TestHost::new(|request| match request["operation"].as_str() {
        Some("messages.list") => Ok(json!([{"id": "1"}])),
        _ => Err("Capability not granted".into()),
    });
    assert_eq!(host.run(crate::colecto_main, "open").unwrap()["text"], "1 messages");
}
```

The mock follows the Host's bridge rules (only the first emit counts), but it does not
enforce fuel, memory, call or size limits. Oneko is also run through the real sandbox
in `crates/colecto-host/tests/http.rs`.
