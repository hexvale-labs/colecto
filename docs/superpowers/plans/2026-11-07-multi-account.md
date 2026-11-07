# Multi-account Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans. Track steps below.

**Goal:** In-app multi-account management, persistent OS credentials, optional password vault, and a productive UI.
**Architecture:** Core owns account metadata and isolated MailStores; Host owns OS credential adapter. Account-bound service clients prevent stale cross-account work. A versioned credential state switches atomically between OS entry references and encrypted vault.
**Tech Stack:** Rust, Axum, Svelte, keyring 3, Argon2id, XChaCha20Poly1305.
**Spec:** docs/superpowers/specs/2026-11-07-multi-account-design.md

## Global constraints

Preserve existing demo drafts and environment support. Secrets never enter API responses or browser storage. No plaintext fallback. Keep local mail unencrypted, explicitly disclosed. Run just verify and just ui-test. No untrusted extensions or new protocols.

## Review focus

- Duplicate message/draft IDs across accounts must stay isolated.
- Failed or interrupted protection changes must retain usable credentials.
- Locked protection must suppress environment credentials too.
- Stale requests must never populate another account's UI.
- Unavailable keychains must produce actionable errors without plaintext fallback.

## Task 1: Credential persistence and account core

Files: create crates/colecto-core/src/accounts.rs and credentials.rs; modify lib.rs, transport credential cleanup, workspace manifests; tests alongside modules.
Interfaces: Accounts::open(MailStore, Arc<dyn SecretStore>), list, upsert, disconnect, store(id); Vault::open, commit, protect, unlock, lock, reset; Secrets remain private to core.
- [x] Add failing tests for registry/store isolation and encrypted vault roundtrip, tampering, wrong password, restart, transitions and keychain failure.
- [x] Run targeted cargo tests and observe failures.
- [x] Implement versioned atomic state and test SecretStore; production adapter remains Host-owned. Use per-account Arc<Mutex<MailStore>>, avoid global network locks.
- [x] Run core suite.

## Task 2: Host account boundary

Files: modify crates/colecto-host/src/{lib.rs,main.rs}; add account integration tests in tests/http.rs; extend extension-api wire types.
Interfaces: GET/POST /accounts; POST /accounts/{id}/disconnect; GET/POST /security; POST /security with a typed action; existing mail routes use account header.
- [x] Add failing HTTP tests for host-only management, isolation, unknown accounts and locked sends.
- [x] Implement validated account selection and management. Preserve app(store,...) compatibility and add persistent startup constructor.
- [x] Run Host tests.

## Task 3: UI management and palette

Files: ui/src/{App.svelte,style.css,lib/api.ts}, new AccountSettings.svelte, composer/reader/mailbox; ui/tests/demo.spec.ts.
Interfaces: createServices(accountId) captures account; hostService manages accounts/security. Shell owns selected account and settings view.
- [x] Add browser tests for switching, settings, vault and draft guard.
- [x] Implement settings, switcher, explicit sender, error states; semantic tokens and compact responsive layout.
- [x] Run just ui-check and browser tests.

## Task 4: Verification and documentation

Files: README.md, docs/README.md, this ledger.
- [x] Document credential backends, vault format/recovery and prototype limits.
- [x] Run just verify and just ui-test; inspect diff.
- [x] Fresh independent review, fix material findings, repeat affected checks.

## Execution ledger

Ruling: implement inline on feat/multi-account in the shared workspace, honoring AGENTS.md autonomy for routine reversible choices. No push or merge.

Task 1: complete — registry, isolated stores, journaled wrapping-key storage and optional Argon2id vault. Twelve focused account/security tests pass.
Task 2: complete — host-only management, account-scoped mail and sanitized secret inputs.
Task 3: complete — settings, sender identity, responsive selector and semantic palette. Startup navigation is gated until remembered identity is restored.
Review: four material findings fixed: startup sender race, corrupt cleanup deletion, unlock lock lifetime, Windows keyring blob limits. The OS store now holds a fixed-size random key; authenticated ciphertext lives in the registry.
Ruling: no merge/push/PR requested; leave the feature branch and local changes for inspection. Native platform interoperability and live provider/phone testing remain explicitly unverified.
