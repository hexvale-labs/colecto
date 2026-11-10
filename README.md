# Colecto

A customizable email client with Rust core services and a Svelte UI. The
single-owner mail service runs inside the desktop Host or as a standalone daemon.
Browse mail, search Inbox/Sent through your provider, and save/reopen local drafts.
The daemon can receive mail while the UI is closed. This remains a prototype;
background receiving currently polls the latest 100 Inbox messages.

The client defaults to dark mode. Open the gear beside the profile at the bottom
of the sidebar for one Settings pane: appearance, accounts and credentials,
server connections, scoped API keys, and extensions. Choose System, Dark, or Light. Account profile
names are optional and fall back to the email address. Opening mail adds closable,
account-specific tabs; Inbox, Drafts, and Sent remain below the open tabs.

See [daemon deployment and notifications](docs/daemon.md) for authenticated remote
HTTPS deployment, local HTTP over Unix sockets, supervisor-independent startup,
and the optional Linux desktop notification agent. Systemd is optional.
Apple and native Android push integration are deferred; a mobile browser layout
is not evidence that native FCM/UnifiedPush or Android background services work.

## Run the local demo

Install stable Rust (1.89 or newer) with `rustfmt` and `clippy`, `just`, and
Bun 1.4.4 or newer. Linux also needs OpenSSL development libraries and pkg-config
(for example, `libssl-dev`, `libdbus-1-dev`, and `pkg-config` on Debian/Ubuntu). From the repository root:

```sh
just setup-ui
just demo
```

Open **http://127.0.0.1:4173**. Stop with Ctrl+C. The demo binds only to IPv4
loopback; open the printed address exactly, rather than `localhost`. The Host
serves the built UI and API from the same origin. The browser prompts for the
owner access token: open the private `.colecto-demo/owner-token` file locally and
copy its contents into that prompt. With `COLECTO_DATA_DIR`, use `owner-token`
inside that directory instead. The Host generates the file with Unix mode `0600`
and tightens its owned data directory to `0700`; it never prints the secret or
puts it in a URL. Do not share or commit this file. An existing non-private token
file is rejected; correct its ownership/permissions before restarting.

The native app (`just native`) connects automatically, injecting its local owner
credential only at the exact local service origin. Remote HTTPS navigation
receives no local credential. Ordinary browsers retain the entered owner token
only for that browser session. In Settings → API keys, create separately scoped
keys for automation, selecting explicit accounts and operations. Copy each key
when created; it cannot be recovered later. Scoped keys cannot manage settings
or directly send mail.

Use **Compose** to write a note, then **Save draft**. Open **Drafts** to resume it.
Leaving an edited draft asks before discarding unsaved changes. Saving failures
keep the editor text, and never report success. Without account configuration, sending is disabled. With an account configured,
**Send** submits plain-text mail through SMTP or JMAP.

The Host stores accounts, drafts, labels, and Inbox/Sent snapshots in one
embedded Turso database, `.colecto-demo/colecto.db`. This directory is ignored by
git. Drafts and mail snapshots are plain text on disk and are not encrypted; use
sample content. Only one Host may open a data directory at a time. A corrupt or
unsupported store stops startup without overwriting it. Preserve that directory
before manually attempting recovery. See [storage](docs/storage.md) for the
record format and one-time legacy JSON import.

Optional environment variables:

```sh
COLECTO_PORT=4180 COLECTO_DATA_DIR=/path/to/demo-data just demo
```

A relative data directory is relative to the command's working directory.
Keep this directory between runs to preserve drafts.

## Check changes

```sh
just verify          # Rust fmt/check/Clippy/tests, Svelte/TypeScript, UI build
just perf-check      # verification plus three recorded Linux resource runs
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
- `colecto-core`: TLS IMAP/SMTP, prototype JMAP, MIME plain-text decoding, sample messages, draft
  validation, IDs, and embedded Turso storage.
- `colecto-host`: authenticated loopback HTTP/static adapter; validates explicit owner bearers,
  scoped API keys, Host/origin, and built-in
  module grants scoped to an explicitly selected account, persisted extension policy,
  and isolated Wasm panels. API bodies are bounded to 512000 bytes.
- `ui/src/extensions`: replaceable bundled mailbox, reader, and composer views.
  The Host supplies their service clients; views cannot access storage directly.

Built-in module identities are routing labels for trusted application code.
Third-party Wasm panels run without WASI under resource limits, start disabled
with no grants, and emit only declarative text/actions. Third-party JavaScript
and HTML are not loaded. An optional Tauri desktop runner uses the same loopback
Host and exposes no guest native IPC. Attachment support, sync and tested mobile
builds are not implemented. See [the extension contract](docs/extensions.md) for
the exact security boundary and `just native` prerequisites.

The frontend build currently inherits a low-severity `deepmerge` advisory from
`@sveltejs/vite-plugin-svelte`, with no upstream fix. The merge runs on trusted
build configuration, not email or API input; track upstream before adding
untrusted build configuration.

## Connect a mail account

IMAP receives mail; SMTP sends mail. **POP3 will not be supported.** JMAP is an
experimental alternative. Add accounts in **Settings → Accounts & security**, or
use the legacy `COLECTO_*` environment variables. Credentials are encrypted and
protected by the OS credential store or an optional master password.

See [accounts and credentials](docs/accounts.md) for provider setup, environment
variables, delivery semantics, and the credential vault.

## Extensions

Bundled mail UI packages have TOML manifests in `extensions/`. The **Settings → Extensions**
Host control shows metadata, enables/disables packages, and limits service
capabilities per account, saved across restarts. It can install isolated
third-party Wasm panels. `just native` starts the optional Tauri desktop runner. See
[the extension contract](docs/extensions.md) for setup and security limits.

## Bun troubleshooting

Run repository tasks with `just`, or run UI scripts from `ui/`, for example
`cd ui && bun run --bun build`. There is no root `package.json`; `bun run build`
from the repository root fails with `Script not found "build"`.

`bun run` is not a test runner: executing the restart test that way fails with
`Cannot use test outside of the test runner`. `just ui-test` runs that test with
`bun test`, then runs Playwright separately. Playwright matches only `*.spec.ts`
to avoid collecting Bun tests. `bun run --bun test` explicitly runs the Playwright
package script; bare `bun test` invokes Bun's own runner instead.

The project uses `ui/bun.lock` and frozen installs. No npm installation is needed.
These failures were reproduced locally; without your original error output,
they do not establish which failure occurred on your machine.

## Resource regression checks

Run `just perf-check` after feature changes. Three isolated workloads record raw
CPU and memory samples, mean/median, and environment/build provenance under
`performance-results/`. Preserve that history for comparisons; see
[resource measurement limits](docs/performance.md). Initial comparisons report
changes without enforcing noisy hardware-dependent budgets.

## Documentation

| Document | Covers |
| --- | --- |
| [Project direction](docs/README.md) | Architecture, terminology, and how agents work here |
| [Accounts and credentials](docs/accounts.md) | IMAP/SMTP/JMAP setup and credential protection |
| [Storage](docs/storage.md) | `colecto.db` records, bounds, and legacy import |
| [Daemon](docs/daemon.md) | Remote HTTPS, Unix sockets, notifications, MCP |
| [Extensions](docs/extensions.md) | Manifests, capabilities, and the Wasm sandbox |
| [Performance](docs/performance.md) | Resource measurement and baselines |
| [Prototype design](docs/prototype-design.md) | The original local demo design |
| [Client review](docs/client-review.md), [UX review](docs/ux-review.md) | UI audits and adversarial fixtures |
| [Specs](docs/superpowers/specs/) and [plans](docs/superpowers/plans/) | Approved designs and their implementation status |
| [Agent instructions](AGENTS.md) | Rules for AI agents working in this repository |
