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
- [ ] Add regression tests for auth, service lifecycle, socket cleanup and background refresh.
- [ ] Extract reusable runtime and standalone binary; preserve host/native entrypoints and old APIs.
- [ ] Add portable launch documentation and sample systemd units; authenticated remote mode and local socket mode.
- [ ] Verify targeted Rust tests, then workspace integration.
### Task 2: Provider mail operations
- [ ] Add tested inbox/sent querying and provider search/capability reuse where current contracts allow.
- [ ] Preserve legacy no-query inbox behavior and account/module grants. Explain fallback limitations.
### Task 3: Client and mobile UX
- [ ] Add focused failing UI flows for theme persistence, name fallback, settings navigation and account-safe tabs.
- [ ] Implement approved client changes, connect service configuration and audit/fix mobile issues.
- [ ] Run Svelte checks/build and browser flows; report physical-device limits.
### Task 4: Resource measurements
- [ ] Add repeatable isolated workloads and three-run resource recorder with history/comparison metadata.
- [ ] Integrate just recipe and usage docs; exercise recorder and retain baseline results.
### Task 5: Integration and review
- [ ] Review UI with emil-design-eng/mobile-native and security/lifecycle boundaries independently.
- [ ] Fix important findings, run just verify and relevant browser checks/resource runs.
- [ ] Update docs/README.md and root README with actual delivered scope and honest remaining limitations.

## Progress / handoff
- Planning: complete. Branch created from clean main. No implementation changes before plan.
- All requested features remain open until checked above; incomplete Android/push/agent work must be recorded rather than silently declared done.
