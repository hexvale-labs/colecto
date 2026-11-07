# Client Productivity Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add local labels, complete search, and keyboard access to client actions.
**Architecture:** Rust persists bounded account metadata atomically. The Svelte shell coordinates guarded actions; a dialog handles command discovery without animation.
**Tech Stack:** Rust, Svelte 5, Playwright; no new dependencies.
**Spec:** docs/superpowers/specs/2026-11-07-client-productivity.md

## Global Constraints

Keep labels local; preserve capability checks, account isolation and unsaved edits. Use just verify and just perf-check.

## Review Focus

- Labels must survive restart and never cross accounts.
- Invalid labels and unknown message IDs must not change storage.
- Async results must not repopulate a departed view.
- Keyboard commands must preserve unsaved-change and busy guards.
- Long identifiers, emoji and narrow layouts must remain usable.

### Task 1: Label core and service boundary

Files: crates/colecto-core/src/labels.rs, lib.rs; crates/colecto-extension-api/src/lib.rs; crates/colecto-host/src/lib.rs; extensions/reader/manifest.toml.
- [x] Add failing persistence, validation and isolation tests; run them.
- [x] Implement bounded versioned label storage and capability-checked endpoints.
- [x] Run targeted Rust tests.

### Task 2: Client productivity

Files: ui/src/extensions/MailShell.svelte, Reader.svelte, Mailbox.svelte; ui/src/CommandRunner.svelte; ui/src/lib/api.ts; ui/src/style.css; ui/tests/workspace.spec.ts.
- [x] Add failing browser tests for labels, draft search and keyboard commands.
- [x] Implement local label management/filtering and reader assignment.
- [x] Add command dialog with keyboard navigation, focus restoration and guarded actions.
- [x] Add search feedback, draft search and invalidate departed requests.
- [x] Add development-only worst-case fixture picker; fix content overflow and avatar segmentation.
- [x] Run browser tests, just perf-check; refresh baseline and document limits.

## Execution notes

- Implemented inline, with one independent read-only review.
- Local labels use a separate versioned file; existing draft/cache formats are unchanged.
- Label mutations and list refreshes cannot overlap in the client, preventing stale reads from undoing newer label state.
- Review corrections: use explicit composer action attributes; restore dialog focus only on dismissal; hide unavailable send commands.
- Browser fixtures for synthetic account IDs also mock labels; real label persistence runs before the existing suite disconnects the demo store.
- Browser suite: 23 passing tests, plus process-restart persistence. Final just perf-check passed; all three isolated samples retained in performance-results/20261107T223022Z-f39db823-497280.json and copied to docs/performance-baseline.json.
