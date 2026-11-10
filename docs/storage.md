# Application storage

The owner service holds an exclusive `store.lock` for its data directory. Before
opening Turso, Core checks directory ownership on Unix and restricts it to mode
0700. It rejects a database symlink. One `colecto.db` holds Core state; account
subdirectories are legacy import/lock locations, never authoritative databases.
Turso uses its embedded Rust engine rather than a SQLite-format substitute.

`Accounts::database()` returns the cloneable shared `Database`. Its internal
connection is serialized behind a mutex. Typed services use bounded, versioned
records (`get_record`, `initialize_record`, `set_record`, `remove_record`) instead
of exposing SQL or a generic KV API to clients/extensions. Namespace and record
names are bounded to 256 bytes, values to 32 MB, and schema versions are positive.
Set/remove reject a different existing schema version. Callers validate their own
serialized schemas. Initialization returns canonical bytes, never stale import
bytes; removal retains a tombstone. Transactional initialization markers prevent
reimport when a previously initialized row is missing. A removed record may be
explicitly replaced by a trusted typed operation, but never by initialization.

Core stores the existing account registry as one versioned encrypted-vault
document, including account metadata and orphan-key cleanup journal. Provider
secrets remain encrypted with XChaCha20Poly1305; account metadata remains its AAD.
The OS keyring holds system keys, and password protection retains Argon2id v19
with its existing parameters. Neither raw system keys nor provider credentials are
written into unencrypted records. Drafts/labels/mail snapshots are local plaintext;
locking the vault clears active readable snapshots, not stored bytes.

Each valid legacy document imports before its own initialization marker commits;
invalid or unsupported documents remain untouched and fail visibly. Account-local
Turso snapshots import through the actual engine, retaining folder/provider IDs
and fetch timestamps. Inbox/Sent snapshots are keyed by account/provider/folder,
with 100 messages and 12 MB aggregate serialized content per account. Oversized,
negative-timestamp or conflicting-provider legacy caches are rejected. Failed
refreshes/writes preserve the last valid snapshot. Provider switches remove prior
provider content transactionally. Searches remain online and do not replace
folder snapshots. Successful IMAP UIDVALIDITY changes invalidate the affected
mailbox's snapshot and retained reader IDs, even for an empty query result.
This is a bounded snapshot cache, not full mailbox synchronization or JMAP state
synchronization. No provider network call runs while the database mutex is held;
independent in-memory snapshots remain readable during account network work.

Migration leaves original files in place without further writes. Canonical state
wins on restart, even when legacy files are later changed or moved aside.
Rollback therefore requires deliberate restoration/export: running an older
binary against preserved legacy files loses changes made after migration.
Host extension policy and Wasm modules, API key verifiers, the send-approval journal
and owner preferences are versioned records in the `host` namespace of the same
database, reached through `Accounts::database()`. A valid legacy `extensions.json`,
`api-keys.json` or `send-approvals.json` imports once, with the same size and
owner-only permission checks as before; the file is then never read or written again.
An invalid legacy file fails startup and stays untouched. Owner preferences hold the
workspace layout (a bounded JSON object the UI validates), the selected account and
whether MCP is on. The browser keeps only the session's access token in
`sessionStorage`. No device replication is introduced.
