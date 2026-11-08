# Mail Client Security Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the approved practical email client and scoped, human-approved automation.

**Architecture:** Extend shared Rust Core services and MailService, keeping Tauri and daemon adapters thin. Preserve the existing account/extension isolation, embed Turso locally, and expose automation through authenticated principals rather than caller-selected extension headers.

**Tech Stack:** Rust, Axum, Svelte 5, Tauri 2, embedded Turso, maintained MIME/cryptography and HTML sanitization libraries.

**Spec:** `docs/superpowers/specs/2026-11-07-mail-client-security-design.md` (approved).

## Global Constraints

- One daemon per OS user; no shared multi-user hosting or cloud database requirement.
- Reuse capability names and shared service checks; scoped keys never become owner/extension identities.
- Human approval is required for automation sends; default expiry is 10 minutes; no automatic retry after uncertain submission.
- Local inbox cache remains bounded to 100 messages and 12 MB serialized content.
- No decrypted private message persistence, secret export, silent plaintext fallback, or fabricated verification checkmarks.
- Preserve existing mail/drafts/grants and fail closed on unsupported persisted policy.
- Use focused RED/GREEN regression tests, `just verify`, and final `just perf-check`; preserve measurement history and refresh baseline.
- Follow repository skills and existing conventions. No speculative abstractions. Update user documentation with actual working limits.

## Review Focus

- A restricted key sets trusted Host headers: it must still be denied owner controls and out-of-scope mail.
- An approval is replayed after restart or revocation: delivery must not start twice or under stale grants.
- A provider returns hostile HTML and misleading sender headers: neither trusted-origin execution nor a false verified badge is allowed.
- An interrupted cache migration or failed refresh occurs: previously stored mail must remain readable.
- A send times out after provider acceptance: show uncertainty without automatically resending.

## File ownership and order

Tasks execute sequentially because they share contracts and service wiring. Task 1 owns scoped principals and key controls; Task 2 owns pending sends and MCP. Task 3 owns cache and refresh/error contracts. Task 4 extends message/draft MIME contracts and composer. Task 5 adds crypto on those contracts. Task 6 validates company sender evidence. Task 7 measures integrated native/resource behavior. All tasks may update their focused tests and documentation; no whole-repo cleanup.

### Task 1: Scoped API keys and owner controls

**Files:** Create `crates/colecto-host/src/api_keys.rs`, `ui/src/ApiKeys.svelte`; modify Host `service.rs`, `lib.rs`, `runtime.rs`, workspace manifests/lock, `ui/src/lib/api.ts`, Settings integration in `ui/src/extensions/MailShell.svelte` or `ui/src/App.svelte`; test `crates/colecto-host/tests/http.rs` and module tests.

**Interfaces:** Produce authenticated `Principal` (owner or API key), `ApiKeys` persistent policy, and key lookup/authorization methods that take current time, account, and existing `Capability`. UI controls use `/api/v1/api-keys` (GET/POST) and `/api/v1/api-keys/{id}` (DELETE). Scoped HTTP authorization is determined from the principal, never supplied extension headers. Reserve scoped `messages.send` for Task 2 approval requests: deny direct delivery until integrated.

- [x] Write failing tests named for key secret nonpersistence, expiry/revocation, explicit account grants, forged Host headers, missing/invalid authentication, and restart persistence. Assert metadata never includes verifier/secret.
- [x] Run focused Host tests and retain expected RED evidence.
- [x] Implement random secret generation, constant-time verifier checks, atomic private policy storage with schema validation, expiry/last-used metadata and bounded names/counts. Owner controls mint keys; keys cannot manage policy or create owner sessions. Preserve remote owner bootstrap, protect local owner controls using a Host-owned session plus Origin/Host checks; no key can downgrade into anonymous loopback privileges.
- [x] Add usable Settings UI with selected accounts/capabilities, optional expiry, copy-once secret and revoke actions. Follow Emil design skill, accessible errors, existing appearance/mobile patterns.
- [x] Run focused tests, `just verify`, and relevant existing UI tests. Update daemon/extension documentation; commit the complete slice.

### Task 2: Pending delivery approval and MCP

**Files:** Create `crates/colecto-host/src/approvals.rs`, `crates/colecto-host/src/mcp.rs`, `crates/colecto-host/src/bin/colecto-mcp.rs`, `ui/src/SendApprovals.svelte`, focused Host MCP/approval tests; modify service/router/runtime, API and Settings, manifests/lock, `docs/daemon.md`, justfile as needed.

**Interfaces:** Consume Task 1 principals and account/capability checks. Produce durable `PendingSend` snapshots and request/status/owner approve/reject service operations, with `pending`, `rejected`, `expired`, `submitting`, `sent`, `uncertain` states. Owner approval performs one submission; scoped `/send` creates pending state instead of direct sending. MCP tools invoke the same services and respect authenticated principals on every call. Add bounded local stdio adapter and authenticated HTTP MCP endpoint, documenting supported stable protocol version.

- [x] Write failing tests for immutable content, approval replay/restart, 10-minute expiry, revocation/removal between request and approval, cross-key request access, client disconnect, and provider-error uncertainty. Use controlled transport/fixture behavior, never real mail.
- [x] Run focused tests to capture RED.
- [x] Implement bounded persistent approval state with atomic transitions before submission. Authenticated owner-only decisions cannot be represented by agent-supplied approval flags. Recheck key/account policy before initiation. Crash during submission becomes uncertain on restart. No automatic retries.
- [x] Implement real MCP initialization/tool discovery/calls with input validation, structured errors, account scoping and size limits, using a maintained SDK if appropriate. stdio must not print secrets/protocol noise; connect with a scoped key through a private credential file or environment and never command-line secret arguments. Prefer quick pending IDs over a long blocking approval request; optional elicitation cannot grant authority.
- [x] Build accessible approval UI showing the exact outgoing snapshot and requester. Add status/rejection handling for API/MCP callers.
- [x] Run focused tests plus a real MCP SDK client interoperability test and `just verify`; document setup/key use and static-token compatibility limits. Commit.

### Task 3: Embedded Turso cache, immediate cached reads, and readable errors

**Files:** Create `crates/colecto-core/src/cache.rs`; modify core `lib.rs`, `accounts.rs`, Host mail routes/refresh services, extension API and TS contracts, MailShell/Mailbox loading states, `ui/src/lib/api.ts`, workspace manifests/lock, cache/provider/HTTP/UI tests, `docs/performance.md` and architecture docs.

**Interfaces:** `MailCacheStore` provides account/provider-bound snapshot load/store with folder and fetched-time metadata. Preserve existing core synchronous consumers with a narrow owned adapter if the embedded API is async; do not introduce nested-runtime panics or unbounded workers. Add cached listing separately from explicit provider refresh while retaining compatible existing message-list responses. Add fetch state and stable safe error categories.

- [ ] Write failing tests for one-time JSON import, provider identity mismatch, rollback source preservation, future schema rejection, transactional failed writes, bounds, and failed refresh preserving latest snapshot. Add non-JSON proxy and typed-safe error UI/HTTP tests.
- [ ] Run focused tests for RED.
- [ ] Embed the current official Rust Turso engine locally; no `rusqlite` substitution or cloud credentials. Migrate valid legacy cache once transactionally and preserve source. Keep account stores and secrets isolated; store only original encrypted content, never decrypted results.
- [ ] Expose cached mail promptly and explicit refreshing/last-fetched/stale state; preserve cached results on fetch errors and allow retry without losing current reader/draft state. Keep list/search/folder semantics explicit.
- [ ] Map known provider failures to readable categories and safe bounded details; do not expose raw protocol bodies/secrets or uncertain sends as safe-to-retry failures.
- [ ] Run focused cache/HTTP/UI tests and `just verify`, update documentation and commit.

### Task 4: Safe HTML reading and complete composer MIME fields

**Files:** Modify extension API, TS contracts, core `remote.rs`, `jmap.rs`, `lib.rs`, draft persistence, reader/composer/shell components and styles. Create focused sanitizer/attachment/composer helpers only where responsibility warrants; add MIME fixtures and provider/UI tests. Read `emil-design-eng` and relevant mobile skill first.

**Interfaces:** Extend `Message`, `Draft`, `DraftInput` compatibly with defaulted HTML alternative, recipient lists/Cc/Bcc, attachment metadata+bounded content and reply threading fields. Update all Rust struct literal callers/tests. Task 2 approvals must snapshot and review every added field automatically or through explicit UI updates. Retain plain text compatibility and old draft readability.

- [ ] Write failing MIME and UI tests for HTML-only/alternative mail, script/form/CSS/network attacks, multiple recipients, Bcc header omission, attachment quotas/removal, reply/reply-all/forward recipients/thread headers, and rich-text draft restart preservation. Add a test ensuring later draft edits cannot mutate pending sends.
- [ ] Run focused tests to capture RED.
- [ ] Implement maintained allowlist HTML sanitization, sandboxed isolated rendering and restrictive CSP with remote resources blocked by default; plain text remains selectable. Do not run mail HTML in trusted origin or introduce unrestricted resource proxies.
- [ ] Implement To/Cc/Bcc, attachments, basic formatting/plain alternative, reply/reply-all/forward end-to-end through draft save/load, validation, SMTP and JMAP. Use existing provider capabilities and explicit readable unsupported errors rather than silently dropping fields.
- [ ] Add accessible controls, dirty-state/error/loading handling, and mobile layouts using existing patterns. Update approval review for all new delivery fields.
- [ ] Run focused provider/core/UI tests and `just verify`; document limits, commit.

### Task 5: OpenPGP and S/MIME

**Files:** Create core `crypto.rs` and focused format helpers/tests/fixtures; modify credentials/account APIs, MIME parsers/senders, Host owner controls, contracts and reader/composer/Settings/approval UI, workspace manifests/lock, docs.

**Interfaces:** Owner-controlled import/fingerprint/sign/encrypt/decrypt services use the existing protected-secret boundary. API keys/Wasm guests cannot export secrets or obtain automatic decrypted plaintext. Automated sign/encrypt is permitted only within a human-approved immutable send snapshot naming the identity and recipient fingerprints. Preserve raw canonical MIME and produce signature/encryption/trust metadata separately from later company verification.

- [ ] Write failing interoperability fixture tests for PGP/MIME and S/MIME signatures/encrypted messages, wrong/untrusted keys, vault lock, key-export denial, requested-encryption failure, and decrypted-cache exclusion.
- [ ] Run focused tests for RED.
- [ ] Use maintained libraries for standard MIME crypto and certificate/path verification; no bespoke cryptographic primitives. Import/display keys and certificates, preserve originals, bound parsing/decompression, and report lock/trust/signature failures clearly. Never downgrade encryption silently.
- [ ] Wire owner UI controls, reader trust states, composer signing/encryption and immutable approval summary without exposing key material. Test API-key/Wasm denial and approved single-use signing at the public service boundary.
- [ ] Run focused tests and `just verify`, document actual supported formats/trust limits and commit.

### Task 6: Evidence-based verified company senders

**Files:** Create core `verification.rs` and controlled DNS/certificate/provider fixtures; modify account trust configuration, owner settings, provider metadata extraction, reader/contract verification fields and safe networking helpers, manifests/lock and docs.

**Interfaces:** Consume original receiving metadata from Task 4/5 without conflating personal signatures with company identity. Produce verified/unknown/failed evidence state with checked/expiry times and explanation; only validated BIMI/VMC plus aligned authenticated sender identity yields a company checkmark. Default is no badge; scope/API-key responses cannot inject verification results.

- [ ] Write failing tests for forged name/logo/Authentication-Results, missing or non-enforcing DMARC/alignment, invalid/expired/untrusted VMC, domain/logo mismatch, private DNS targets/redirects and valid controlled evidence. Test trust-policy changes and evidence expiry invalidate cached badges.
- [ ] Run focused tests for RED.
- [ ] Implement explicit trusted receiving-authentication configuration and bounded SSRF-safe DNS/HTTPS evidence fetching, certificate chain/policy/domain/logotype validation using maintained libraries. Never trust a sender-controlled header alone or equate a regular TLS certificate with VMC.
- [ ] Wire readable verification details and company checkmarks into reader with accessible/mobile layout. Unknown or unsupported evidence gets no checkmark. Owner-only trust configuration cannot be altered by scoped keys/guests.
- [ ] Run focused boundary/UI tests and `just verify`, document supported evidence/trust/revocation limitations honestly and commit.

### Task 7: Native resource measurement, justified optimizations, integrated checks

**Files:** Modify `scripts/resource_bench.py`, recorder tests/workloads, justfile, Tauri/runtime/core hot paths only where measurements support a change, docs/performance.md and baseline. Create `docs/performance-investigation.md` for sources, experiments, retained/rejected decisions.

**Interfaces:** Extend recorder modes to real Linux Tauri/WebKit process trees with comparable metadata, separate workload/build/runtime identities, idle/startup/read/compose/refresh CPU/RSS. Keep existing Chromium and daemon history comparisons separate. No universal performance budget or inferred platform gains.

- [ ] Preserve preimplementation `just perf-check` samples. Write failing recorder tests for native process aggregation, invalid comparison metadata and cleanup; run them for RED.
- [ ] Research primary Tauri/WebKit/Rust resources. Inspect connection creation, IMAP fetching, cache allocation, runtime worker counts, render/poll work, and release profiles. Record hypotheses and complexity before changing candidates.
- [ ] Add native measurement and take matching baseline/candidate samples. Keep only repeatable improvements whose maintenance cost is justified; record unsuccessful/reverted candidates and distinguish binary size from runtime usage.
- [ ] Run `just verify`, relevant complete UI/native tests and final `just perf-check` (three isolated samples). Keep timestamped history and copy the final integrated JSON to `docs/performance-baseline.json`. Configure artifact retention only when CI exists.
- [ ] Independently review the integrated branch, resolve important findings, rerun affected checks and final measurement if code changes affect it. Commit measurement/docs; report verified results and untested environments plainly.
