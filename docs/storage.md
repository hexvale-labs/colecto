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
database, reached through `Accounts::database()`. The Host keeps these records only in
`colecto.db` and ignores old JSON files. Owner preferences hold the
workspace layout (a bounded JSON object the UI validates), the selected account and
whether MCP is on. The browser keeps only the session's access token in
`sessionStorage`. No device replication is introduced.


Folder discovery also stores the last successful folder list in the account's
versioned `folders` record. The record includes the provider identity, so changing
accounts or provider settings cannot expose a previous provider's folders.
`MailStore::folders()` falls back to that list when discovery fails; a failed
discovery never overwrites it. Counts describe the last successful discovery,
not necessarily the current server state.

Account removal commits the registry and secret deletion before cleaning local
storage. It deletes `snapshots` and `mailbox_identity` rows for that account and
tombstones all records in `account:{id}` (drafts, labels and folders), retaining
initialization markers to prevent legacy reimport. Cleanup is transactional and
retry-safe through `Database::remove_account_data`; a cleanup failure does not
restore the removed account. Other accounts and mail-server data are untouched.
