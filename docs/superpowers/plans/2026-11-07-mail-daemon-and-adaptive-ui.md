# Mail daemon and adaptive UI implementation plan

> **For agentic workers:** Use superpowers:executing-plans for implementation, with explicitly authorized parallel agents for independent subsystems. Keep this file current across compaction.

**Goal:** Extract a single-owner deployable mail service, modernize the client, and retain reproducible resource measurements.
**Architecture:** Reuse Rust core services and versioned extension contracts. A foreground daemon supports embedded/local TCP, authenticated remote HTTP behind TLS, and HTTP over Unix sockets. Desktop supervisors are optional adapters. Android uses application lifecycle and optional FCM/UnifiedPush; uninterrupted Android execution is not promised.
**Tech stack:** Existing Rust/Axum/Tokio, Svelte 5, Bun, Playwright, just.
**Spec:** Approved conversation, summarized below. User explicitly said get to work; no further design approval required.

## Approved scope and constraints
- Single owner per daemon, multiple accounts/devices; defer multi-user/admin/bootstrap-password design.
- Daemon first: reusable runtime, background receiving, credentials, sending, remote serving, Unix socket ownership/cleanup, supervisor-independent foreground execution. Preserve existing stores and extensions' grants.
- Remote mode must authenticate clients; never expose current loopback header identities as authentication. TLS terminates at a documented reverse proxy or equivalent trusted layer.
- Local desktop service can outlive UI. Remote desktop notifications require an optional session agent, not a second upstream sync service. Keep lifecycle distinct.
- Android supports FCM and UnifiedPush; Apple is deferred. Android local execution is best effort under OS policy. Do not claim push or mobile native integration exists without implemented platform code and validation.
- Reuse JMAP and IMAP/SMTP capabilities. Add supported fallbacks with honest semantics, not speculative provider emulation. Persist identifiers and account isolation.
- Dark default; System/Dark/Light settings. Optional per-account profile name defaults to address. Bottom avatar initial/name/settings row replaces sidebar management clutter; account switching remains reachable.
- One settings pane for appearance, accounts, server connections, security, extensions. Scrollable open-email sidebar tabs, separator, Inbox/Drafts/Sent. Preserve unsaved drafts and account-bound tabs.
- Mobile UX audit with emil-design-eng and mobile-native; implement findings. Keyboard, focus, contrast, touch/scroll, error recovery; record hardware-only validation limits.
- Resource checks: three measured runs, raw samples plus mean/median, peak/retained memory and CPU time, timestamps/commit/machine/build profile; comparable-run baselines, durable history/artifacts, initial report-only thresholds.
- Use just recipes. Run just verify for Rust changes and relevant UI/browser checks. No unsolicited dependencies/abstractions. Branch: feat/mail-daemon-and-adaptive-ui. Work in existing checkout to preserve shared tool workspace.

## Review focus
- Authenticated remote users cannot bypass authorization with extension/account headers or leak tokens in URLs/logs.
- Unix socket startup never deletes a live or unrelated socket/file; shutdown removes only owned socket.
- Failed receive/send and daemon shutdown preserve drafts, credentials, and last successful mail state.
- Theme/tab/account switching preserves edits and handles long names and narrow screens accessibly.
- Benchmark comparisons identify incompatible environments and never confuse browser memory with daemon memory.

## Tasks and verification
### Task 1: Daemon foundation
- [x] Add regression tests for auth, service lifecycle, socket cleanup and background refresh.
- [x] Extract reusable runtime and standalone binary; preserve host/native entrypoints and old APIs.
- [x] Add portable launch documentation and sample systemd units; authenticated remote mode and local socket mode.
- [x] Verify targeted Rust tests, then workspace integration.
### Task 2: Provider mail operations
- [x] Add tested inbox/sent querying and provider search/capability reuse where current contracts allow.
- [x] Preserve legacy no-query inbox behavior and account/module grants. Explain fallback limitations.
### Task 3: Client and mobile UX
- [x] Add focused failing UI flows for theme persistence, name fallback, settings navigation and account-safe tabs.
- [x] Implement approved client changes, connect service configuration and audit/fix mobile issues.
- [x] Run Svelte checks/build and browser flows; report physical-device limits.
### Task 4: Resource measurements
- [x] Add repeatable isolated workloads and three-run resource recorder with history/comparison metadata.
- [x] Integrate just recipe and usage docs; exercise recorder and retain baseline results.
### Task 5: Integration and review
- [x] Review UI with emil-design-eng/mobile-native and security/lifecycle boundaries independently.
- [x] Fix important findings, run just verify and relevant browser checks/resource runs.
- [x] Update docs/README.md and root README with actual delivered scope and honest remaining limitations.

## Progress / handoff
- Planning: complete. Branch created from clean main. No implementation changes before plan.
- All requested features remain open until checked above; incomplete Android/push/agent work must be recorded rather than silently declared done.

### Integration record
- Independent review completed by a fresh agent using ponytail-review/emil-design-eng/mobile-native. Three Important findings accepted: transactional cache preparation, long fallback-name edits, and cancellation of queued background operations at shutdown.
- Fixed cache/account consistency by preparing validated transports and snapshots before vault commit, then applying the prepared connection without I/O. Regression `corrupt_mail_cache_cannot_commit_new_credentials_or_account_settings`: RED → GREEN.
- Preserve blank optional names in storage; all display surfaces use address fallback. Regression `address_longer_than_profile_limit_can_be_edited_without_a_profile_name`: RED → GREEN. UI action labels and disconnect confirmation use the same fallback (additional regression).
- Stop queued refresh permit acquisition on shutdown; only already-dispatched provider operations finish. Regression `queued_refresh_never_starts_after_shutdown`: RED → GREEN.
- Added mailbox-qualified Sent IMAP IDs and a real IMAP wire regression. This revealed/fixed pre-existing invalid multi-attribute FETCH syntax. JMAP wire test proves upstream Sent/text search and requested ordering.
- Bounded combined text bodies and reader cache bytes (12 MB) in addition to the 500-ID cap. Memory-bound regression tests: RED → GREEN.
- UI agent checks: full Playwright 16/16, final focused 5/5, Svelte/build clean. No physical-device validation. Root integrated checks in progress.
- Benchmark agent checks: five Python tests and real three-run browser/daemon workloads passed. Root will record a stable integrated baseline and refresh the committed development sample.
- Delivery decision: remote server selection opens that server's same-origin UI; simultaneous aggregation is not implemented. One-owner bearer auth replaces the earlier discarded multi-user/admin design.
- Platform follow-up: native Android lifecycle/FCM/UnifiedPush registration and sending are not implemented; preserve these requested choices for the Android platform slice. Apple deferred. Background provider work currently uses bounded polling, not full-history incremental sync, provider IDLE, labels/flags, or offline mutation queues.


### Final verification
- `just perf-check`: passed. Workspace tests: 59 passed across core, account persistence, HTTP/security, daemon, Unix runtime, and unit suites. Formatting, compilation, strict Clippy, Svelte check and production build passed. Resource recorder tests: 5 passed; three measured runs retained locally.
- `just ui-test`: passed, including real Host restart persistence and 17 Playwright flows. Root added a regression for unnamed-account action labels and confirmation text (RED → GREEN).
- `just native-check`: passed. This checks native compilation, not an Android build or physical-device behavior.
- Final additional `just perf` records the final UI label fix; committed baseline is refreshed from that run. No hardware-independent memory budget is claimed.
- Remaining explicitly deferred scope: native Android application/service integration and both push transports; provider IDLE/incremental full-history synchronization, labels/flags and offline mutations; simultaneous multi-server aggregation. Apple remains outside initial support. Password-vault mode needs unlocking after restart; remote deployment requires external HTTPS termination.
- Branch stays local for review; no merge, push, deployment, or PR requested.
