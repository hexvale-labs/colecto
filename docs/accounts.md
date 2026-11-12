# Accounts and credentials

Account setup, provider transports, and credential protection for the local
Host and daemon. Storage layout is described in [storage](storage.md).

## Connect a mail account

IMAP receives mail; SMTP sends mail. **POP3 will not be supported.**
Use **Settings → Accounts** inside the app to add IMAP/SMTP or JMAP accounts, edit
settings, reconnect, and disconnect. The account selector stays available on
narrow screens. Inbox, drafts, and the compose sender belong to the selected
account. Saving settings does not verify the provider connection: open Inbox to
check it. Connection errors keep local drafts available.

Passwords/tokens persist as authenticated ciphertext, protected by a random key
in your operating system credential store: macOS Keychain, Windows Credential
Manager, or a running Secret Service on Linux. Linux builds
need `libdbus-1-dev`. If the store is missing or locked, Colecto reports an error;
there is no plaintext fallback. Account management requires the local trusted
Host; it is not a hosted multi-user authentication system.

For an existing environment-based setup, the following variables remain supported
before `just run` (set secrets in your local environment, never in source control):

| Variable | Meaning |
| --- | --- |
| `COLECTO_IMAP_HOST` | IMAP server hostname |
| `COLECTO_IMAP_PORT` | Implicit TLS port, default `993` |
| `COLECTO_SMTP_HOST` | SMTP server hostname |
| `COLECTO_SMTP_PORT` | `587` (required STARTTLS, default) or `465` (implicit TLS) |
| `COLECTO_MAIL_USERNAME` | Login for both servers |
| `COLECTO_MAIL_PASSWORD` | Password or provider app password for both servers |
| `COLECTO_MAIL_ADDRESS` | Sender email address |

Any mail variable enables account mode; missing or invalid settings stop startup.
Certificates and hostnames are verified; plaintext authentication is unavailable.
When unlocked, a legacy environment account is imported once into persistent
account settings and the system credential store, retaining its existing `demo`
identity and drafts. Saved settings then take precedence; use Settings → Accounts to
update them. A password-protected vault suppresses environment credentials while
locked; reconnect/import through the UI after unlocking. Secrets are never
returned through the API or saved in the draft store. OAuth-only providers are not supported yet.

Opening Inbox fetches the latest 100 messages, read-only, without marking them
seen. Reopen Inbox to refresh. Messages larger than 2000000 bytes are skipped;
HTML-only messages show a placeholder. The latest successful Inbox and Sent
snapshots persist in the shared `colecto.db`, bounded to 100 messages and 12 MB
per account. They are plain text, keyed to provider identity, and restored on
reconnect; receiving failures preserve the previous snapshot. Search results are
held in a bounded in-memory reader cache. Search uses upstream IMAP TEXT or JMAP
text filters and returns at most 100 matches. Sent uses the provider role, with
conventional IMAP folder-name fallbacks. SMTP acceptance does not automatically
append a Sent copy. Drafts remain local and unencrypted, namespaced by account
inside `colecto.db`. The original `demo` account keeps its legacy identity and
drafts; they are never assigned to a new account automatically. Disconnect
forgets the saved credential and retains settings/drafts for reconnecting.

Send success means SMTP acceptance, not final delivery. A failed or interrupted
send can have an uncertain outcome: check the provider's Sent folder before
retrying. No automatic retry, SMTP Sent-folder append, or saved-draft deletion is
performed. The editor preserves text on failure and disables sending again after
success. Server/network operations run outside the async executor and serialize per
account, with 30-second socket timeouts. Another account can continue independently.
Credential changes and locking report busy while an account operation is active.

Transport libraries follow the [imap client documentation](https://docs.rs/imap/2.4.1/imap/)
and [lettre SMTP documentation](https://docs.rs/lettre/0.11.23/lettre/transport/smtp/).

## JMAP accounts

JMAP ([RFC 8620](https://www.rfc-editor.org/rfc/rfc8620),
[RFC 8621](https://www.rfc-editor.org/rfc/rfc8621)) is a full alternative to
IMAP/SMTP: one HTTPS connection receives, searches and sends mail, with push.
In **Settings → Accounts**, choose **JMAP** and enter:

- **JMAP server or session URL:** the server's address, such as
  `https://mail.example.com`, or its exact session URL. A bare address uses
  `/.well-known/jmap`. Redirects are followed only to HTTPS on the same host, so your
  credentials never go to another server. Plain HTTP is accepted only for a server on
  this machine (`localhost`, `127.0.0.1`, `[::1]`).
- **Username:** leave it empty to sign in with an **API token** (Fastmail and other
  providers that issue bearer tokens). Fill it in to sign in with a **password** or app
  password instead (Stalwart, Cyrus, Apache James and other servers using HTTP Basic).
- **Email address:** the address to send as. Colecto uses the identity with that
  address, then a `*@your-domain` wildcard identity, then the account's only identity.

For a single account from the environment, set `COLECTO_JMAP_SESSION_URL`,
`COLECTO_JMAP_TOKEN` (the token or password), `COLECTO_MAIL_ADDRESS` and optionally
`COLECTO_JMAP_USERNAME`, and leave the IMAP/SMTP variables unset. Partial
configuration fails startup; authentication and capability checks happen on first
use. OAuth sign-in and token refresh are not implemented.

What works, the same as IMAP/SMTP:

- The primary mail account's latest 100 messages per folder, fetched without
  marking them seen, in one request (`Email/query` with a back-referenced
  `Email/get`). Plain text up to 100000 bytes per part is listed; the raw message is
  downloaded on demand for HTML and attachments.
- Search through the server's `text` filter.
- Push through the session's event source: a change to the account's `Email` state
  refreshes it within seconds, and polling continues underneath.
- Sending with Cc, threading headers and attachments. Attachments are uploaded as
  blobs first (respecting the server's `maxSizeUpload`), the message is created in
  Drafts and submitted, and on success the server moves it to Sent and clears
  `$draft`. Sending needs the submission capability and mailboxes with `drafts` and
  `sent` roles. If the server refuses the message, the error names its reason (for
  example `tooLarge`).

Submission acceptance is not final delivery, and failure can leave a server draft or
an uncertain send; check Sent and Drafts before retrying. There are no automatic
retries. HTTP responses are limited to 8000000 bytes, with 30-second timeouts.

`just test jmap` (also run in CI) sends a message with an attachment through a real
JMAP server, the Cyrus test server in Docker, then receives it, searches for it,
downloads the attachment and finds it in Sent. Push is covered by unit tests, because
that server does not emit push events.

## Optional master password

In **Settings → Accounts → Credential protection**, enable password protection with a
master password of at least 12 bytes. This is optional: the default is your
system credential store. Password mode also works when no desktop keychain is
available. Keep the password somewhere safe: Colecto cannot recover it.

Password protection encrypts **credentials only**, not email bodies, local drafts,
or connection metadata. The account registry, a versioned record in `colecto.db`,
contains settings and an encrypted credential vault. Default mode references an OS entry holding a
fixed-size random encryption key, so long provider tokens do not exceed Windows
credential blob limits. Password mode derives its key from your master password. OS entries are
namespaced by the canonical data directory. Moving that directory requires
reconnecting accounts in system-store mode; retain a backup before moving it.

The vault uses Argon2id v19 (64 MiB, 3 iterations, 1 lane) and XChaCha20Poly1305,
with random salt/nonces and authenticated account metadata. The password and
derived key are not saved. It locks on Host restart; **Lock credentials** also
removes active mail credentials from memory. Local drafts remain accessible.
Unlocking is shared by tabs using the same local Host. A tab that changes account
security elsewhere may need to reopen Settings → Accounts to refresh its status.

You can change the password while unlocked or return to the system credential
store. Credential transitions journal replacement entries before switching the
registry and retry obsolete-entry cleanup after interruption. If cleanup fails,
the UI reports pending cleanup; unlock the OS store and retry. A startup cleanup
failure stops the Host with an actionable error rather than silently leaving a
protection transition incomplete. Preserve the data directory on corruption;
Colecto does not overwrite unsupported or malformed records or legacy files.

**Reset saved credentials** forgets credentials for all accounts without deleting
local drafts. It requires confirmation and every account must then be reconnected.
This is the recovery path for a forgotten password, not password recovery.

The implementation follows the official [keyring API](https://docs.rs/keyring/3.6.3/keyring/),
[Argon2 API](https://docs.rs/argon2/0.5.3/argon2/), and
[authenticated encryption API](https://docs.rs/chacha20poly1305/0.10.1/chacha20poly1305/).
OS-keychain behavior on macOS/Windows, live provider connections, and real phone
interaction require platform testing; automated tests use an injected credential
store or a password vault with sample connection settings.

## Reading and sending

The list and offline snapshots still hold decoded text only. Opening a message
calls `GET /messages/{id}/content` (`messages.read`), which fetches the raw
message from the provider without marking it seen and returns full text, HTML,
attachment metadata, and `Message-ID`/`References`/`Reply-To`/`To`/`Cc`.
Messages above 30 MB are refused. Messages above 2 MB are listed from headers
and only load when opened. If the provider is unreachable the reader falls back
to the saved text, and replies are sent without threading headers.

HTML is untrusted. The reader renders it in an `<iframe sandbox>` with
`srcdoc`: no scripts, forms, plugins or same-origin access, and links open in a
new tab. The frame's own policy blocks remote images until the owner chooses
**Load remote images**; the Host policy allows inline styles and `https:` images
only so that frame can render. `cid:` inline images are not shown yet.

`GET /messages/{id}/attachments/{index}` returns one attachment as
`application/octet-stream` with an attachment disposition, so it can never
render in the app origin.

Sending accepts comma-separated `recipient` and `cc`, `in_reply_to` and
`references`, base64 `attachments` (at most 20 files and 25 MB total; the
`/send` route accepts bodies up to 36 MB), and `forward_of`, which re-attaches
the original message's attachments on the server and also requires
`messages.read`. Drafts keep recipients, Cc, threading and `forward_of`, but not
uploaded files. Delivery requests from API keys cannot include attachments or
forwards, because owner approval shows text only. JMAP sends threading headers,
multiple recipients and attachments.

### Untrusted message and push limits

Raw MIME messages are checked before recursive decoding: at most 32 nested levels
below the root and 1000 total parts. Messages exceeding either limit fail decoding,
including during background refresh. Body and attachment readers share this check.
Malformed non-ASCII IMAP folder IDs return a missing-message error.

Disconnect, vault lock/reset and account reconfiguration revoke push watcher
credentials without waiting, and revoked watchers cannot start another wait. A push
wait already in progress (IMAP IDLE or a JMAP event stream, up to ten minutes) cannot
be interrupted, so it keeps its own credential copy until it returns; the copy is then
dropped. Lock therefore does not guarantee that no credential remains in memory until
that wait ends.
JMAP creation errors show fixed descriptions for recognized error types and a
fixed generic description for unknown provider values.

### Composer and automation review safety

Send-approval notifications open the immutable request in Settings → Send approvals;
only the review card showing recipients and body offers approval. Keyboard focus moves
to the requested card. Rejection remains available from the notification.

The composer reserves attachment count and bytes as soon as files are selected
(at most 20 files and 25 MB). Send waits for all reads and shows a reading status.
Read failures block sending until attachments are cleared and selected again.
Reply and forward prefills, including headers and quote prefixes, are cut to 80,000
UTF-8 bytes between Unicode code points, leaving 20,000 bytes of the 100,000-byte body
limit for the owner's own text.

## Account settings dialog

In Settings → Accounts, select **Add account** beneath the compact account list.
The dialog groups email and optional account name under Identity, protocol and
hosts under Server, and username and password under Sign in. Username starts from
the email address; JMAP API-token sign-in can leave it empty. Show password reveals
the credential only in the dialog. Cancel and Escape ask before discarding edits.

Each saved account has Reconnect, Edit and Remove icon buttons. Reconnect and Edit
open the same prefilled dialog; leave the credential blank to keep it. Remove asks
before disconnecting and forgetting the saved credential. The account record,
local mail and drafts remain. Protocol tags expose server hosts on hover. Connected
means the Host reports a live connection; the page does not invent a last-check time.
Credential protection and master-password recovery remain below the account list.
