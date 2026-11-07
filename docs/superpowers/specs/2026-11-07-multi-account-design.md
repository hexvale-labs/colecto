# Multi-account mail and productive UI

Status: approved by the user, including credentials-only optional encryption.

## Intent and scope

Manage multiple mail accounts inside Colecto, switch quickly, and preserve the
correct account for every read, draft, and send. Keep the existing Rust core,
loopback Host, Svelte UI, and IMAP/SMTP and JMAP transports. Refresh the visual
system for everyday productive use, with restrained colors and immediate navigation.

The user chose in-app account management and persistent sign-in using the system
credential store, and requested optional password encryption. This design treats
that option as an app-wide master password protecting mail credentials. Message
and draft encryption is outside this change; the UI must say so explicitly.

## Approaches

- External configuration plus a switcher: fewer controls, but does not meet the
  user's choice of in-app management.
- In-app management with session-only secrets: simpler persistence, but requires
  signing in after restart. The user chose persistent sign-in instead.
- In-app management with system credential storage, plus an optional master
  password vault: selected direction. More security and recovery handling, but
  supports both easy startup and an explicit app lock.

## Accounts and compatibility

The Host exposes typed account list, add, update/reconnect, and disconnect
operations to trusted host controls. Bundled mailbox, reader, and composer
operations retain explicit account scope. They cannot retrieve secrets or manage
credentials. Validate account identifiers and grants at the Host boundary.

Use stable opaque account identifiers and a versioned metadata registry without
secrets. Keep separate stores and locks for each account; scope lookups, caches,
draft IDs, network operations, and sending to the selected account. A slow account
must not hold a global lock during network work and block another account.

Preserve the legacy store and its drafts under its existing account identity;
never assign existing drafts to a newly connected account automatically. Preserve
legacy environment configuration for compatibility. Do not overwrite malformed or
unsupported metadata. Use atomic writes and keep identifiers stable across restart.

Disconnect removes the saved credential and disables remote operations, retaining
local drafts and account metadata. Reconnect restores the same account identity.
Permanent deletion of local mail is outside this change.

## Credentials and optional encryption

Default mode protects passwords and bearer tokens with the operating system
credential store. Implementation decision: keep a fixed-size random wrapping key
there and authenticated credential ciphertext in the registry; this supports long
provider tokens within Windows credential blob limits. Namespace OS entries by
the canonical data directory and a random generation ID. The registry
contains display/connection settings and encrypted credential ciphertext. A missing, locked, or unavailable
system store produces an actionable error, never a plaintext fallback. Secrets
are never included in account API responses, logs, or browser storage.

An optional "Protect credentials with a master password" setting replaces saved
system-store secrets with an encrypted credential vault. Use a maintained Rust
implementation of a memory-hard password KDF and authenticated encryption, with
random salt and nonce and a versioned format. Select concrete libraries and
parameters during implementation against their official documentation. Do not
implement cryptography from primitives.

Do not save the master password or its derived key. Unlock once per Host session;
restart locks the vault. A lock control drops usable credentials from the Host
and is disabled while a credential mutation or send is in progress. Wrong
passwords or damaged ciphertext do not change the vault. There is no password
recovery; resetting credential protection requires reconnecting accounts and does
not delete local drafts.

When vault protection is enabled, suppress automatic loading of legacy
environment credentials while locked. Environment settings must never bypass the
lock; importing them into protected persistence requires an unlocked session.

Changing password or enabling/disabling protection must persist the replacement
successfully before removing the previous copy. A crash may leave the previous
protected copy, but must not lose credentials or silently downgrade protection.
On startup, finish any pending credential cleanup before declaring a protection
transition complete. Disabling encryption requires unlocking the vault and a
working system credential store. The UI explains that this protects credentials,
not locally stored message or draft content.

## Account and navigation UI

Place a labeled account switcher in the sidebar, visible on narrow screens as
well. Show account name/address, connection state, and a direct Manage accounts
action. Management provides protocol-specific connection fields, masked secrets,
reconnect, disconnect, and credential protection controls. Never report a
connection as verified before a provider check succeeds; failed checks preserve
entered settings without exposing secrets.

Switching accounts or entering settings preserves the existing dirty-draft
confirmation. Block navigation during save/send. Clear the reader and list on a
successful switch and reject stale responses from the previous account. Bind
service clients to the account that initiated an operation, rather than reading
a mutable global selection when asynchronous work completes. Show the sender
account prominently in compose and retain text after failure.

Keep Inbox, Drafts, Compose, refresh, and account controls directly reachable.
Do not add unsupported folders, decorative counters, or a unified inbox in this
change. On narrow screens, retain a focused list/detail flow and accessible Back.

## Visual system

Use semantic CSS tokens rather than repeated color literals:

- Canvas: warm white `#FAFAF8`; surfaces: `#FFFFFF`.
- Separators and muted surfaces: `#E8EAE5`.
- Primary text: ink `#252B27`; secondary text: `#626B64`.
- Primary actions: forest `#315C45`, with white text.
- Selected rows: pale green `#EAF1EB`, with dark text and a visible selection mark.
- Error: dark rust text on a light warm surface; never color alone.

These are proposed defaults, not user-specified exact colors. Confirm text and
focus contrast during implementation. Use the existing system sans-serif stack,
compact readable rows, consistent spacing, restrained corners, and explicit
button labels. Remove oversized decorative empty-state content in favor of a
short useful instruction. Use visible keyboard focus and native semantics.

Navigation and keyboard actions are immediate. No list entrance effects or
decorative motion. Any small control transition remains brief and respects
reduced motion. Apply touch hover gating, adequate targets, 16px mobile inputs,
dynamic viewport sizing, and safe-area padding where applicable.

## Validation

Cover account isolation using overlapping message/draft IDs, wrong-account
lookups, sending identity, account permission failures, and restart persistence.
Use a test credential backend so automated checks do not modify a real keychain.
Cover vault round trips, wrong password, tampering, unavailable keychain,
enable/disable/password changes, and interrupted credential transitions.

Browser tests cover account management, dirty draft cancellation, rapid switching
with stale responses, sender identity, lock/unlock, errors, keyboard navigation,
and narrow layouts. Run `just verify` and the relevant `just ui-test` flows.
Report actual OS-keychain, live-provider, and phone checks separately; mocked
checks cannot establish platform interoperability.

## Boundaries

No OAuth login/refresh, provider autodiscovery, device sync, untrusted extension
runtime, native Tauri packaging, attachment support, encrypted mail/draft store,
or new mail protocol. Keep this as the existing browser/loopback application;
system credential operations belong in its local trusted Host.
