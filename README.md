# Colecto

A customizable email client with Rust core services and a Svelte UI. This first
prototype runs locally in a browser: browse sample mail, read messages, and
create, save, reopen, and edit drafts. Drafts survive restarting the Host.

## Run the local demo

Install stable Rust (1.89 or newer) with `rustfmt` and `clippy`, `just`, and
Node.js 22.12 or newer with npm. From the repository root:

```sh
just setup-ui
just demo
```

Open **http://127.0.0.1:4173**. Stop with Ctrl+C. The demo binds only to IPv4
loopback; open the printed address exactly, rather than `localhost`. The Host
serves the built UI and API from the same origin.

Use **Compose** to write a note, then **Save draft**. Open **Drafts** to resume it.
Leaving an edited draft asks before discarding unsaved changes. Saving failures
keep the editor text, and never report success. There is no send button: no mail
leaves your computer.

The Host saves drafts to `.colecto-demo/drafts.json`, using format version 1.
This directory is ignored by git. Drafts are plain text on disk and are not
encrypted; use sample content. Only one Host may open a data directory at a time.
A corrupt or unsupported store stops startup without overwriting it. Preserve
that directory before manually attempting recovery.

Optional environment variables:

```sh
COLECTO_PORT=4180 COLECTO_DATA_DIR=/path/to/demo-data just demo
```

A relative data directory is relative to the command's working directory.
Keep this directory between runs to preserve drafts. The demo has no data-format
migration mechanism yet; a future incompatible format must provide one.

## Check changes

```sh
just verify          # Rust fmt/check/Clippy/tests, Svelte/TypeScript, UI build
just setup-browser   # install the test Chromium browser once
just ui-test         # browser flows and actual Host restart persistence
```

The browser tests run a separate Host on port 4179 with temporary sample data.
They do not use your `.colecto-demo` drafts. Playwright may require system browser
libraries on some Linux installations. The tests cover a narrow viewport, but
real phone touch/keyboard behavior has not been verified.

Run `just` for all recipes. After editing UI source, `just demo` rebuilds it.

## Prototype boundaries

- `colecto-extension-api`: version 1 wire types for the operations used today.
- `colecto-core`: sample messages, draft validation, IDs, and atomic JSON storage.
- `colecto-host`: loopback HTTP/static adapter; validates Host/origin and built-in
  module grants scoped to the demo account. API bodies are bounded to 128000 bytes.
- `ui/src/extensions`: replaceable bundled mailbox, reader, and composer views.
  The Host supplies their service clients; views cannot access storage directly.

Built-in module identities are not an authentication or sandbox mechanism.
Third-party code is never loaded. There is no extension discovery/lifecycle
runtime, native Tauri host, real account connection, receiving/sending, attachment
support, sync, or tested mobile platform build yet. Tauri v2 remains the intended
native host; this slice chooses a local Rust HTTP backend for the browser.

The frontend build currently inherits a low-severity `deepmerge` advisory from
`@sveltejs/vite-plugin-svelte`, with no npm-provided fix. The merge runs on trusted
build configuration, not email or API input; track upstream before adding
untrusted build configuration.

See [project direction and architecture](docs/README.md), the
[approved prototype design](docs/prototype-design.md), and [agent instructions](AGENTS.md).
