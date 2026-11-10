# Single Turso application store

Status: implementation requested by the owner on 2026-11-08 with instructions to continue. This amends the approved mail-client security design's earlier decision to leave non-cache JSON stores in place.

## Ownership and storage

Each OS-user service owns one private embedded Turso database, `colecto.db`, in its data directory. Core and Host share a database handle; account identifiers namespace data inside that database rather than creating account databases. Tauri and the daemon use the same Rust services. Remote frontends use the selected service's database through authenticated APIs; this does not add device replication or shared multi-user hosting. Native client mode must be able to target the running user service without opening an incidental second database/listener; an explicit launch server URL is acceptable bootstrap input, and receives no automatic local owner credential.

Persist accounts and the encrypted credential vault, drafts, local labels, mail snapshots, installed extension manifests/modules, enablement/grants, API-key verifiers/metadata, pending sends, durable daemon metadata, and user preferences there. A small namespaced key/value table with versioned JSON or blob values is appropriate for existing bounded document-shaped records; use typed service APIs and parameterized SQL, not arbitrary SQL/KV access from extensions or MCP clients. Reuse current schemas and validation rather than unnecessarily normalizing every document.

OS keyring secrets, password-derived keys, the private owner bootstrap credential and process-lock files retain their security/bootstrap roles outside the database. Do not put plaintext provider passwords, private keys, raw API-key secrets, owner/session bearer credentials, or decrypted inbound message bodies into an unencrypted KV row. The encrypted vault payload and API-key hashes belong in Turso. Session credentials remain ephemeral browser session state, not application settings.

In-memory snapshots are derived caches. They must never overwrite newer canonical database state or restore revoked permissions. Do not retain JSON/localStorage as a second writable source of truth.

## Migration

Import existing valid accounts/vault, drafts, labels, extension policy/modules, API keys, approvals and account-local `mail-cache.db` data. Preserve original files as migration/rollback evidence without continuing to write them. Validate versions, limits and identities before committing imported records. Record migration markers transactionally; retry safely after interruption, and never overwrite canonical data from stale legacy files. Missing/corrupt/newer authoritative security records fail closed. Changes to accounts and credential protection preserve the existing encryption/AAD and orphan-key cleanup invariants.

Database and WAL/journal files must be protected by a private, ownership-checked data directory before writes. Keep process ownership exclusive at the service boundary. No network call may run while holding the shared database mutex/transaction. Cached reads must remain available while provider network work holds an account lock.

## Preferences and daemon state

Owner-only settings APIs persist validated appearance and selected-account preferences plus supported application settings in Turso. Import known legacy browser preferences once only when the canonical preference is unset; then remove their legacy keys. Browser localStorage must not remain authoritative. Do not persist transient loading/error flags or recover unsent work by automatically delivering it.

The daemon reads/writes the same account, permission and cache services without a UI. Persist meaningful durable notification baselines/provider-cache metadata when appropriate; never treat a failed poll as a successful new baseline. Do not introduce a second KV engine or a separate writable daemon cache file.

## Mail cache policy

Keep account/provider/mailbox identities distinct. IMAP cache identities include UIDVALIDITY and UID; never reuse sequence numbers as durable identifiers. Invalidate obsolete mailbox-identity data after a successful namespace change. A bounded recent-folder result is a snapshot, not full-history synchronization; JMAP mutable state/query results must be refreshed and cannot be inferred from an old body cache.

Retain original encrypted content and bounded fetched message content/metadata for offline provider access, with fetch timestamps/stale indicators and the existing aggregate 100-message/12 MB content budget per account. Searches stay online unless explicitly implemented otherwise. Failed refreshes preserve the last valid snapshot. Intentional Disconnect/vault Lock still clears active readable snapshots. Provide an owner-only cache-clear action with meaningful scope; database row deletion is not a secure-erase promise.

Block remote message resources by default; do not prefetch/cache tracking URLs, remote styles or arbitrary links just because a message references them. Decrypted inbound content stays ephemeral and owner-controlled. Existing delivery requests remain immutable, expiring and at-most-once initiated; offline cache support never implies automatic resend.

## First-test readiness

After storage consolidation and the originally approved client work, verify fresh startup and real process restart, account/vault setup, preference persistence, installed extension/grant persistence, drafts/labels/cache, API-key scopes and approval replay. Run the required Rust/UI/native checks and final resource measurements using resource preflight and one Cargo job on this machine. Preserve timestamped history and update the integrated performance baseline. Provide a concise first-run/test checklist with exact commands and actual limitations; do not claim a real-provider send was tested without credentials and explicit delivery authorization.

## Primary sources consulted

- [IMAP4rev2, RFC 9051](https://www.rfc-editor.org/rfc/rfc9051.html): persistent UID identity and immutable message data versus mutable mailbox state.
- [Disconnected IMAP client synchronization, RFC 4549](https://www.rfc-editor.org/rfc/rfc4549.html): UIDVALIDITY checks and conservative synchronization.
- [JMAP Mail, RFC 8621](https://www.rfc-editor.org/rfc/rfc8621.html): state/query synchronization and cached email data.
- [Thunderbird remote content guidance](https://support.mozilla.org/en-US/kb/remote-content-in-messages): remote content can disclose message viewing; block automatic loading.
