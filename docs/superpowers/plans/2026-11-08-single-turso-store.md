# Single Turso Store Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make one private Turso database authoritative for Core, Host, daemon and supported UI settings.

**Architecture:** Share a small embedded database handle and namespaced versioned records; preserve existing domain validation/encryption and derived in-memory snapshots. Import old files transactionally and stop writing them. Expose settings/cache maintenance only through existing owner-authenticated boundaries.

**Tech Stack:** Existing Rust/Turso0.7.2/futures-executor, Axum, Svelte5/Tauri2.

**Spec:** `docs/superpowers/specs/2026-11-08-single-turso-store.md`, amending the approved mail-client security design.

## Global Constraints

- One database `colecto.db` per OS-user service; no per-account/daemon KV database or cloud requirement.
- Keep OS keyring/master/bootstrap secrets outside plaintext application rows; encrypted vault and API-key hashes inside Turso.
- Preserve legacy files, validate before import, transactionally mark migrations; never re-import stale sources over canonical state.
- Missing/newer/corrupt security records fail closed; no permission reset or automatic resend.
- Preserve account/provider isolation and 100-message/12 MB per-account cache bounds, independent cached reads, owner-only controls and scoped-key content redaction.
- Preflight effective resource limits/CPU/disk/load before heavy jobs; `CARGO_BUILD_JOBS=1`, no overlapping builds, no benchmark under pressure.
- Keep the original HTML/composer/crypto/verified-sender/performance tasks pending and resume them after this storage amendment.

## Review Focus

- A stale legacy file is modified after migration: canonical database state must win.
- Revoked policy or account encryption state is missing/corrupt: startup/access must fail closed, never restore defaults.
- Import fails halfway: original files remain intact and retry produces the same authoritative records without duplication.
- UI and daemon use the same database: supported settings and mail state survive real process restart.
- Cached data survives network failures without crossing account/mailbox UID identity or persisting decrypted/remote tracker content.

### Task 1: Shared database and Core state migration

**Files:** Create `crates/colecto-core/src/database.rs`; modify core `lib.rs`, `accounts.rs`, `credentials.rs`, `labels.rs`, `cache.rs`, manifests if needed, core/mail/account tests, and architecture/storage documentation.

**Interfaces:** Export cloneable `Database` (owned private path and shared bounded connection/mutex) plus small record get/initialize/set/remove operations. Versioned namespaced records support existing serialized schemas. `Accounts::database()` returns the shared handle; child MailStores receive that handle plus account ID, never open their own authoritative DB. `MailStore::open` retains a compatibility entrypoint for tests/standalone construction. Mail snapshots use an account/provider/folder primary key in the same database. Constructors may retain legacy directory paths solely for source import and process locking.

- [x] Write failing tests proving drafts/labels/vault/account metadata and multi-account snapshots survive reopening from `colecto.db` after legacy sources are moved aside; import preserves originals and ignores later stale edits.
- [x] Run focused RED tests. Include schema rejection, transactional import failure, account-cache isolation/bounds and vault encryption/AAD/keyring cleanup regression cases.
- [x] Implement private single-database schema/record access and inject it through Core stores. Validate records before import and keep migration markers atomic. Persist encrypted vault bytes rather than secrets; retain keyring protection and process locks.
- [x] Migrate legacy account-local Turso cache rows using the actual Turso engine, preserving valid timestamps/identity and account-wide quotas. Integrate IMAP mailbox-identity invalidation for retained reader data when a successful UIDVALIDITY change is observed, with controlled fixtures; retain honest bounded-snapshot/JMAP refresh semantics.
- [x] Run focused Core/account/mail tests then `CARGO_BUILD_JOBS=1 just verify` when stable. Update docs with actual storage/migration boundaries, commit only this task, record evidence and integration interfaces.

### Task 2: Host policies, daemon state and UI preferences in Turso

**Files:** Modify Host `api_keys.rs`, `approvals.rs`, `extensions.rs`, `service.rs`, `runtime.rs`, `main.rs`, `lib.rs`; add a focused settings module/component only if needed; modify extension/TS contracts, `theme.svelte.ts`, MailShell/App settings flows, Host/HTTP/MCP/restart/UI tests, root README and daemon/cache docs.

**Interfaces:** Consume Task1 `Database` shared from Accounts. Host records contain existing versioned key-policy, approval and extension data, including installed module bytes. Provide owner-only `GET/POST /api/v1/settings` with typed appearance/selected-account/cache preferences and owner-only cache clear. Expose no generic KV/SQL endpoint. Session/bootstrap secrets remain ephemeral/protected. Meaningful daemon notification baselines use account/provider-scoped records in the same DB. An explicit `COLECTO_SERVER_URL` native-client launch mode loads a selected HTTPS or loopback HTTP service without opening a second local database/listener; it must never inject a local owner secret into that selected service. Existing embedded mode keeps its exact-origin local bootstrap. Transport/bootstrap environment is deployment input, not a second application KV store.

- [ ] Write failing tests for legacy policy migration, missing/corrupt canonical policy fail-closed, revocation/replay across restart, extension module/grant persistence, settings ownership/validation and shared daemon cache behavior. Add a browser process-restart preference test and removal of legacy localStorage authority.
- [ ] Run focused RED tests.
- [ ] Move Host record reads/writes to transactional Turso records and inject the shared handle in production. Keep compatibility constructors narrowly for tests, without a new database per subsystem. Preserve identity/grant rechecks and durable send state before transport.
- [ ] Implement validated owner settings persistence/loading and one-time browser preference import only when unset, then remove legacy keys. Keep transient loading/errors and bearer session credentials out of durable preferences. Add bounded owner cache-clear UI/action; clear derived readable copies and avoid secure-erase claims.
- [ ] Persist appropriate daemon cache/notification metadata while preserving failed-poll baseline and provider identity semantics. Document the caching policy with primary-source links and add a first-testing checklist/just commands. Include native connection to the user daemon without a competing store owner; validate origin/URL and test that external-client mode creates no local DB or secret injection.
- [ ] Run `CARGO_BUILD_JOBS=1 just verify`, relevant complete UI/restart/MCP checks and native check when supported, sequentially after preflight. Keep final integrated perf-check/baseline for the original plan's Task7 after remaining features. Commit and report exact readiness/remaining work without claiming live provider delivery.
