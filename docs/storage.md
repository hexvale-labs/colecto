# Application storage

Everything Colecto keeps lives in one embedded Turso database, `colecto.db`, per data
directory. Its schema is versioned and migrated forward on startup, and settings store
only what the owner changed.

## Data directory

| Path | Contents |
| --- | --- |
| `colecto.db` | The only authoritative store (Turso, SQLite file format) |
| `colecto.db.before-v<N>` | Copy taken before migrating from schema version `N`; only the newest is kept |
| `store.lock` | Exclusive lock: only one Host may open a data directory |
| `accounts/<id>/store.lock` | Per-account lock; account data itself is in `colecto.db` |

Where the directory is:

| Runs as | Default (`COLECTO_DATA_DIR` overrides) |
| --- | --- |
| Desktop app | The platform's data directory joined with `com.kogical.colecto`: `$XDG_DATA_HOME` or `~/.local/share` on Linux, `~/Library/Application Support` on macOS, `%APPDATA%` on Windows |
| Android app | The app's data directory, `/data/user/0/com.kogical.colecto` |
| iOS app | The app's Application Support directory |
| `colecto-daemon` from the systemd unit | `~/.local/state/colecto` |
| Docker image | `/data` |
| `colecto-daemon` started by hand | `.colecto` in the working directory |

These are the directories the Tauri app (v0.9.0 and earlier) used, so its `colecto.db`
opens and migrates forward after an update.

Before opening Turso, Core checks directory ownership on Unix and restricts it to mode
`0700`. It rejects a database that is a symbolic link.

## Schema and migrations

`crates/colecto-core/migrations/` holds numbered SQL files (`0001_initial.sql`, …),
embedded in the binary by `crates/colecto-core/src/database.rs`. `PRAGMA user_version`
counts the steps already applied, and `PRAGMA application_id` is `0x434F4C45` ("COLE")
on every Colecto database.

On open, `Database::open`:

| Database | What happens |
| --- | --- |
| Empty file | Every step runs; the database is marked as Colecto's |
| Colecto's, older schema | Copied to `colecto.db.before-v<N>`, then each missing step runs in its own transaction |
| Colecto's, current schema | Opens as is |
| Colecto's, newer schema | Refused: "made by a newer Colecto". Downgrades are not supported |
| Tables but no Colecto `application_id` (Colecto alpha or another program) | Refused; move the file aside to start fresh |

### Adding a migration

1. Add the next file, for example `0003_contacts.sql`, and list it in `MIGRATIONS`
   in `database.rs`. Never edit a migration that has shipped.
2. Convert data the owner made (drafts, labels, accounts, keys, settings) in SQL;
   `0002_tables.sql` shows how to read the JSON of a `records` row with `json_each`. Caches
   (`snapshots`, `message_content`, `mailbox_identity`, the `folders` record) can be
   emptied instead, since a refresh refetches them.
3. Run `cargo test -p colecto-core --test single_database -- --ignored write_schema_fixture`
   and commit the new `crates/colecto-core/tests/fixtures/schema/v<N>/colecto.db`.
   `every_schema_fixture_opens_and_keeps_its_data` opens every fixture with the current
   build and checks its drafts, labels, settings, API keys and send approvals survive.
   Extend the sample data when a migration converts something the fixtures do not hold
   yet.

Turso is still pre-1.0 and pinned to an exact version in `Cargo.toml`, so an upgrade is
deliberate and the fixture test shows whether it still reads every schema version.

### Reading the file with SQLite

Best effort, not a promise: Turso writes the SQLite file format, so the stock `sqlite3`
tool can open `colecto.db` (stop Colecto first). `sqlite_can_read_the_database` checks
this in CI and skips where `sqlite3` is not installed. Do not turn on Turso-only
features (concurrent writes/MVCC, encryption, vector indexes) without checking that
they keep the file readable this way.

## Tables

| Table | Key | Holds |
| --- | --- | --- |
| `records` | `namespace`, `name` | Small documents owned by Core or the Host, as bytes (see below) |
| `settings` | `scope`, `key` | Settings the owner changed; `value` is JSON |
| `drafts` | `account`, `id` | One row per draft; `id` is the number in `draft-<id>` |
| `labels` | `account`, `name` | Local labels, at most 50 per account |
| `label_messages` | `account`, `label`, `message` | Messages in each label, at most 1000 per label, in the order they were added (`rowid`) |
| `api_keys` | `id` | API key metadata and the SHA-256 `verifier` of the secret; `accounts` and `capabilities` are JSON arrays |
| `send_approvals` | `id` | Send-approval journal: the reviewed message (`snapshot`, JSON) and its `status`; rows older than 24 hours are deleted |
| `snapshots` | `account`, `provider`, `folder` | Cached Inbox/Sent listings with `fetched_at` |
| `mailbox_identity` | `account`, `provider`, `folder` | IMAP `UIDVALIDITY` and similar identities that invalidate a snapshot |
| `message_content` | `account`, `provider`, `id` | Raw cached messages, oldest evicted first |

### Records

`Database` exposes records through `get_record`, `set_record`, `initialize_record` and
`remove_record`, never raw SQL to clients or extensions. Namespace and name are at most
256 bytes, values at most 32 MB. Callers validate their own serialized formats.

| Namespace | Name | Contents |
| --- | --- | --- |
| `core` | `vault` | Account registry and the XChaCha20-Poly1305 credential vault (format version 1 inside) |
| `account:<id>` | `folders` | The last successful folder discovery |
| `host` | `extensions`, `extension-settings`, `extension-storage` | Disabled extensions and installed packages' manifests, extension settings the owner changed, each extension's key-value store |
| `host` | `ai-summaries` | Saved AI summaries |
| `extension-packages` | `<extension id>` | Installed package bytes |

A missing record reads as empty or default and is not written until something changes.
Records that only the Host reads accept unknown fields, so a field added later does not
break them. The vault rejects unknown fields, because its account list is the
authenticated data of the encryption.

### Settings

Defaults live in code: `Preferences` in `crates/colecto-host/src/preferences.rs`
(scope `core`), `AiSettings` in `ai.rs` (scope `ai`), and extension manifests for
extension settings. The database holds only keys the owner changed, so changing a
default in a release reaches everyone who never changed that setting.

- Keys are dotted paths to a setting, such as `notifications.position` or
  `text_spacing.letter`. A nested struct is split into its fields; a map that starts
  empty (`accounts_enabled`, AI `labels`) or a value that starts as `null`
  (`workspace`, `accent`) is one key.
- A save stores the keys whose value changed, even when the new value equals today's
  default: the owner chose it. `POST /api/v1/preferences` takes `reset: ["key", …]` to
  forget keys so they follow the default again; an empty `accent` does the same.
- On load, a stored key the build no longer knows, or a value that no longer
  validates, is skipped with a warning and the default applies. Settings never stop
  Colecto from starting.
- `GET` and `POST /api/v1/preferences` return the preferences plus `changed`, the keys
  stored now. Settings shows a reset for a group once one of its keys is in `changed`.
- Extension settings follow the same rule: `PUT /api/v1/extensions/{id}/settings`
  stores the values sent, and the owner-only
  `POST /api/v1/extensions/{id}/settings/reset` with `{"keys": […]}` forgets them.

The helpers are `load_settings` and `save_settings` in `crates/colecto-host/src/store.rs`.

## Credentials

The OS keyring holds the vault's system key, and password protection uses Argon2id v19
(64 MiB, 3 iterations, one lane). Provider secrets stay encrypted with
XChaCha20-Poly1305 with the account metadata as associated data. AI API keys are
`host:` secrets inside the same vault. Neither raw keys nor provider credentials are
written into plain records. Drafts, labels and mail snapshots are local plaintext;
locking the vault clears readable snapshots from memory, not stored bytes.

## Mail snapshots

Inbox/Sent snapshots are keyed by account, provider identity and folder, with at most
100 messages and 12 MB of serialized content per account. Failed refreshes or writes
keep the last valid snapshot. Switching provider settings removes the previous
provider's content in one transaction. Searches stay online and never replace folder
snapshots. A changed IMAP `UIDVALIDITY` invalidates that mailbox's snapshot and
retained reader IDs, even for an empty result. This is a bounded cache, not full
mailbox or JMAP state synchronization. No provider network call runs while the
database mutex is held.

Folder discovery stores the last successful folder list in the account's `folders`
record together with the provider identity, so changing account or provider settings
never shows another provider's folders. `MailStore::folders()` falls back to it when
discovery fails; counts describe that discovery, not necessarily the server now.

## Account removal

Removal commits the registry and secret deletion first, then deletes the account's
`snapshots`, `mailbox_identity`, `message_content`, `drafts`, `labels` and
`label_messages` rows and every record in `account:<id>` in one transaction (`Database::remove_account_data`, safe to retry). A
cleanup failure does not restore the account. Other accounts and the mail server are
untouched.
