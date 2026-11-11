# Project direction

Colecto will be an email client built with Tauri v2, Rust business logic, and a
Svelte UI. Deep customizability is a primary product goal, including extensions
that change composing, reading, layouts, and workflows. Target web, desktop, and
mobile from the start when choosing boundaries, without claiming platform parity
before testing it.

## Core services and extensions

Call the trusted foundation **Core services**. Call optional functionality and UI
packages **Extensions**; first-party UI packages are **Bundled extensions**.
Call the small environment that starts the app, loads extensions, and mediates
access the **Host**. These names describe responsibilities, not a mandate to
create a separate crate for every service.

Receiving uses IMAP and sending uses SMTP, or JMAP handles both in the prototype. POP3 will not be supported.

Core services own accounts and credentials, receiving and storing mail, reading
and querying messages, composing drafts and sending mail, settings persistence,
and eventually device synchronization. Reading UI and composing UI belong to
extensions; message access, draft persistence, and delivery belong to the core.
The client must preserve mail and account integrity even when an extension fails.

Expose small, typed, versioned APIs for those services. Extensions request access
through the Host; they never access credential stores, databases, unrestricted
networking, or Tauri commands directly. Grant explicit capabilities scoped to
operations and accounts, with user consent for sensitive access. Validate every
request at the trusted boundary, including bundled extensions' requests.

The app UI should be composed of replaceable bundled extensions, using the same
public contracts as third-party extensions. Keep only bootstrapping, permission
prompts, recovery, and other essential host controls outside that composition.
The Host owns extension discovery, manifest/API compatibility checks, capability
grants, lifecycle, isolation, and failure recovery. Disabling an extension must
not disable core services or prevent recovery.

Third-party panels run in a fuel- and memory-limited Wasmi runtime without WASI.
They use a Host-bound, account-scoped service bridge and emit declarative text
and actions; they never execute HTML or JavaScript in the app origin. Bundled
Svelte packages remain trusted application code. Tauri capabilities alone do not
isolate extension code sharing a privileged context. See the extension contract
for exact limits and platform validation status.

## Platforms and dependencies

Keep business logic in Rust and presentation in Svelte. Native platform adapters
belong in the Host, not in the domain core. Tauri v2 hosts the desktop and mobile
apps; the browser build needs its own adapter to Rust services, using Wasm where
supported or a Rust backend for operations browsers cannot perform. A browser
cannot simply call native Tauri APIs or directly replace native mail transports.
Choose the web deployment model when implementing the first platform slice.

See the official [Tauri overview](https://v2.tauri.app/start/) and
[project structure](https://v2.tauri.app/start/project-structure/) for native host
and frontend boundaries.

Device synchronization is planned for later. Keep persistent identifiers and
storage ownership clear; document compatibility when adding persisted formats.
Do not build a sync engine, speculative sync traits, or promise cross-device
support now. Credentials need a separate explicit security policy before sync.

## Repository and daily work

All Rust crates live under `crates/`, in one root Cargo workspace. The first local prototype implements `colecto-core` mail services and
`colecto-extension-api` version 1 contracts, consumed by `colecto-host` and the
Svelte bundled UI. This browser slice uses a loopback Rust HTTP backend; an optional
Tauri desktop runner and isolated Wasm panels use the same service boundary. See the root
README for run commands and the exact prototype limits.

Use justfiles as the entry point for repeatable operations. Keep shared tasks in
the root `justfile`; introduce component justfiles only when useful. `just` lists
commands; `just verify` checks formatting, compilation, Clippy, and tests.

New crates under `crates/` join through the workspace member glob. Inherit
`version`, `edition`, `publish`, and workspace lints as the existing manifests do.
Declare shared dependency versions, features, and local paths once under root
`[workspace.dependencies]`; consumers opt in with `dependency-name.workspace = true`.
For example, a future Host can use `colecto-core.workspace = true`. Listing a
workspace dependency does not make every crate depend on it. Keep the dependency
graph intentional: Host depends on core and extension contracts; contracts do
not depend on the Host or private core implementation. Commit root `Cargo.lock`.

## Working as agents

This is an AI-agent-run project. Make and explain routine technical decisions;
think through failure modes and maintainability rather than producing volume.
Prefer working vertical slices, existing patterns, small modules, and explicit
ownership. Introduce abstractions only when real callers need them. Avoid empty
frameworks, speculative extension points, duplicated helpers, unused dependencies,
and generated boilerplate that nobody can maintain. Remove superseded code.

Always look for helpful installed skills before starting work. Read applicable
skills and use them deliberately. For UI, use Emil Kowalski's installed skills,
especially `emil-design-eng`, with relevant motion, accessibility, and mobile
skills. Select domain skills for other tasks rather than forcing a UI workflow.

Agents may ask for direction when product intent is missing or a costly decision
cannot be reversed easily. Do not abuse that option: the maintainer may not know
better. Investigate first, make reasonable reversible choices, state assumptions,
and ask only when the answer materially changes the work.

Keep checks proportional: test behavior, boundaries, and meaningful regression
risks; run relevant existing checks. Report what was verified and what remains
unimplemented. Never describe a planned sandbox or platform as working.


## Multi-account slice

The approved [multi-account design](superpowers/specs/2026-11-07-multi-account-design.md)
adds per-account stores and account-bound bundled service clients. The core owns
registry/vault rules; the Host supplies the operating system SecretStore adapter.
Only trusted host controls manage accounts or credential protection. Existing
mail operations retain their module grants and explicit account identifier.
Secrets never cross into read APIs or bundled extensions. This continues to be a
local prototype, not a remote multi-user service. The Wasm panel sandbox is
described separately below.

## Bundled extensions

See [extension manifests and capabilities](extensions.md) for the TOML package
contract, runtime controls, and current trust boundary. Bundled mail UI now loads
through an explicit registry; the trusted Host retains credential and recovery
controls. Third-party Wasm panels start disabled with no grants. Permissions and packages
persist across restarts; an optional Tauri runner exposes no guest native IPC.


## Daemon and client workspace

The approved continuation is recorded in
[the daemon and adaptive UI plan](superpowers/plans/2026-11-07-mail-daemon-and-adaptive-ui.md).
One owner may connect multiple accounts and devices. Multi-user hosting and its
admin bootstrap flow are deferred. `colecto-host` now provides a reusable service
runtime and a standalone `colecto-daemon` binary; this avoids duplicating core
services or introducing a circular crate dependency. The daemon runs in the
foreground under any supervisor; optional systemd units are packaging only.
Remote access uses an explicit HTTPS public origin and an owner bearer token,
with TLS termination at a trusted reverse proxy. Local Unix HTTP sockets use
private directories and ownership-safe cleanup. See [deployment](daemon.md).

Core accounts, the encrypted credential vault, drafts, local labels, and Inbox/Sent
snapshots persist in one owner-local embedded Turso database, `colecto.db`.
Account identifiers namespace records and snapshots inside that file; child mail
stores share its connection handle. Host extension policy, API keys, send approvals and
owner preferences (workspace layout, selected account, MCP switch, accent colour and
notification choices) live in the same
database; no application configuration is written as JSON files. The OS keyring, password-derived keys and process locks
retain their roles outside Turso. Credential AAD and cleanup journals are preserved.

Snapshots are
bounded together to 100 messages and 12 MB serialized content per account.
Provider identities remain isolated; a successful new-provider refresh evicts the
previous provider snapshot in the same transaction. Both folders remain cached
while within that budget; refreshing a folder evicts the
other folder only when needed to meet the aggregate bound. Valid legacy `accounts.json`, `drafts.json`, `labels.json`, account-local
`mail-cache.db` and `mail-cache.json` sources import once after validation.
Record initialization and migration markers commit transactionally; original files
remain as rollback evidence and are never updated. Canonical database state wins
over stale sources. Missing initialized credential records, unsupported versions
and corrupt vaults fail closed. Database rows and retained initialization markers
prevent stale imports from restoring removed state. A rollback to an older binary
requires a deliberate export/restore: preserved sources do not contain later edits.
See [storage](storage.md) for the API and migration boundary.
Cached lists and retained message reads use independent snapshots during network
refresh. Provider searches remain online and do not replace full-folder snapshots. They are not encrypted. Provider-backed Inbox/Sent search and a bounded
reader cache preserve open-message access when changing views. A successful IMAP
UIDVALIDITY change invalidates the affected mailbox snapshot and retained reader
data, including empty/search responses; failed provider queries preserve data. The Host polls
every connected account and also keeps one push watcher per account whose last refresh
succeeded: IMAP IDLE on a read-only Inbox, or JMAP push through the session's
`eventSourceUrl`. A reported change refreshes that account at once and emits a `mail`
event; servers without IDLE or push stay on polling. This is not a complete incremental
sync engine. Provider flags, labels,
full-history pagination, and offline mutations still need explicit contracts.

The client uses a dark-only workspace of dockable panes that split like a game engine's
editor (see `DESIGN.md`), Settings as a card over the workspace,
optional account profile names, and account-bound open-message tabs. Mobile layout is checked
in browser tests; Android-native lifecycle, FCM, UnifiedPush, battery use, and
physical-device keyboard behavior remain separate platform validation work.
Apple is outside the initial platform scope. Resource regression work uses
`just perf-check`; see [performance](performance.md).

## Client productivity

The desktop workspace exposes a searchable **Ctrl/Cmd+K** command runner. Pages the runner
can open (folders, compose, every Settings section) come from one navigator,
`ui/src/lib/navigation.ts`, which MCP `app.navigate` shares through
`NAVIGATION_TARGETS` in the extension API. Search follows typing after a short pause
and never refreshes the whole folder; clearing it restores the saved snapshot. In-app
and desktop notifications for new mail and send approvals follow the owner's
notification settings. Local
account labels support create, assign, unassign, delete and filtering of current
mail results. Inbox/Sent queries use the provider; drafts search recipient,
subject and body locally. Labels do not synchronize with IMAP/JMAP. See
[client review](client-review.md) for the adversarial preview and motion audit,
and [label contracts](extensions.md#local-labels) for limits and permissions.
