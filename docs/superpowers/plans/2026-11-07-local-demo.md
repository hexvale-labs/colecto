# Local Email Demo Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Runnable sample inbox, plain-text reader, and restart-safe drafts.
**Architecture:** Svelte bundled UI modules call a versioned Rust Host API. Core owns validation and persistence; the loopback Host owns grants and HTTP validation.
**Tech Stack:** Rust, serde, axum, tokio, tempfile; Svelte, TypeScript, Vite, Playwright.
**Spec:** `docs/prototype-design.md` (approved by maintainer).

## Global Constraints

- Keep mail logic in Rust, UI in Svelte, dependencies in root workspace declarations.
- Sample account only; no sending, sign-in, untrusted extensions, or platform parity claims.
- Atomic versioned draft storage; never silently replace corrupt data.
- Use justfile recipes; run `just verify`.

## Review Focus

- Failed saves preserve the previous saved draft and the editor text.
- Unknown IDs do not create replacement drafts accidentally.
- Corrupt and unsupported stores fail without rewriting data.
- Cross-origin and ungranted requests cannot write drafts.
- Narrow screens, empty subjects, and unsaved navigation remain usable.

### Task 1: Core and contracts

**Files:** `crates/colecto-extension-api/src/lib.rs`, `crates/colecto-core/src/lib.rs`, `crates/colecto-core/tests/mail.rs`, workspace manifests.
**Interfaces:** `MailStore::open(PathBuf)`, `messages()`, `message(&str)`, `drafts()`, `draft(&str)`, `save_draft(DraftInput)`; public serialized `Message`, `Draft`, `DraftInput`, and `ApiError`.

- [x] Write failing integration tests for sample reads, create/update/reopen, unknown IDs, invalid inputs, exclusive access, corrupt/versioned stores, and failed writes.
- [x] Run `cargo test -p colecto-core`; confirm missing behavior.
- [x] Implement typed contracts and atomic persistence with a single-process store lock.
- [x] Run core tests and formatting.

### Task 2: Loopback Host

**Files:** `crates/colecto-host/{Cargo.toml,src/lib.rs,src/main.rs,tests/http.rs}`.
**Interfaces:** `/api/v1/messages`, `/api/v1/messages/{id}`, `/api/v1/drafts`, `/api/v1/drafts/{id}`; supplied extension identity and operation/account grants.

- [x] Write request tests for successful save/read and denied origin, Host, missing grant, malformed input, unknown IDs.
- [x] Run tests to confirm missing Host behavior.
- [x] Implement API, loopback binding, static frontend serving, bounded requests, JSON errors, and browser response headers.
- [x] Run Host and core suites.

### Task 3: Bundled UI and runnable slice

**Files:** `ui/` build config, `src/App.svelte`, `src/lib/api.ts`, `src/extensions/{Mailbox,Reader,Composer}.svelte`, styles, browser tests; root justfile and README.
**Interfaces:** supplied typed service clients connect bundled modules to Host; editor reports dirty state to shell.

- [x] Write browser tests for inbox/read, create/save/reopen/update, unsaved navigation, save failure, narrow layout, plain-text rendering, and host restart.
- [x] Observe failure before frontend implementation.
- [x] Implement accessible responsive UI, explicit save states, and draft/error/empty recovery.
- [x] Add `just setup-ui`, `just demo`, `just ui-check`, `just ui-test`; update run instructions and limitations.
- [x] Run `just verify`, frontend build/check, browser tests, and restart verification.
- [x] Review whole diff against approved spec and record actual results below.

## Execution record

Implementation authorized inline by “Approve and implement”; no further process approval needed.


- Core: 6 tests passed; Host: 2 request tests passed.
- `just setup-ui` reproduced the locked install; `just verify` passed formatting,
  workspace check, Clippy with warnings denied, Rust tests, Svelte check (0 errors,
  0 warnings), and the production UI build.
- `just ui-test` passed 5 browser scenarios and 1 actual Host process restart test.
- Desktop and narrow-layout screenshots inspected locally; no visual overflow
  found. Muted text colors strengthened for contrast.
- Fresh read-only review found a narrow-screen hidden draft-open error. Added a
  failing browser regression, moved the error into the visible mailbox, and ran
  the full UI suite successfully.
- Ruling: keep the scoped prototype on a local HTTP Host, per approved design;
  Tauri and an untrusted extension runtime remain explicitly deferred.
- Minor review limit: detail navigation does not explicitly move keyboard focus;
  native controls remain keyboard-operable. Physical phone behavior is untested.
- npm reports 2 low-severity entries for one transitive build-time `deepmerge`
  advisory with no available fix; documented the affected trusted input boundary.
- Changes remain in the current feature worktree for maintainer review; no merge,
  push, or publication was requested.
