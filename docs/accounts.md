# Accounts and credentials

Account setup, provider transports, and credential protection for the local
Host and daemon. Storage layout is described in [storage](storage.md).

## Connect a mail account

IMAP receives mail; SMTP sends mail. **POP3 will not be supported.**
Use **Settings → Accounts** inside the app to add IMAP/SMTP or JMAP accounts, edit
settings, reconnect, and disconnect. Adding an account is a three-step wizard (the
same one first-run onboarding uses):

| Step | What happens |
| --- | --- |
| **Email** | Enter the address. **Continue** looks up its server settings (see below); **Enter settings myself** skips the lookup. |
| **Servers** | Protocol, servers, ports, security, username and password. If the lookup found settings, a note above the fields names the provider and every standard used; if it found nothing, no note appears. |
| **Review** | A short summary of where mail comes from, where it is sent, and who signs in, with **Change** links back to each field, warnings for likely mistakes (port 25, TLS on port 143, an SMTP host in the IMAP field), and the optional account name. |

**Back** returns to earlier steps without losing anything. Changing the address on the
first step only updates server fields the owner has not edited; when the new address
finds nothing, every server field stays as it was. A username that was never edited
follows the address. Editing an existing account opens on **Servers** and never lets a
lookup overwrite saved values.

Each port is a joined control: the usual ports (IMAP `993`, `143`; SMTP `465`, `587`,
`25`; JMAP `443`, `8443`) plus an **Other** box that accepts any port from 1 to 65535.
Picking IMAP `993` or `143` also sets TLS or STARTTLS; a custom port keeps the chosen
security. For JMAP, the chosen port is written into the session URL. The account selector stays available on
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
| `COLECTO_IMAP_PORT` | Implicit TLS port, default `993` (STARTTLS needs Settings → Accounts) |
| `COLECTO_SMTP_HOST` | SMTP server hostname |
| `COLECTO_SMTP_PORT` | `465` uses implicit TLS; any other port (default `587`) requires STARTTLS |
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

## Finding server settings

`POST /api/v1/accounts/discover` with `{ "address": "…" }` (owner only) returns the
settings the Host found, or `null`. It runs on the Host, because the interface's
content security policy only allows its own origin. Methods, in the order their
answers win:

| Method | Request | Notes |
| --- | --- | --- |
| Built-in provider list | none | Gmail, Yahoo Mail, iCloud Mail, Fastmail (IMAP and JMAP), GMX, mailbox.org, Posteo, Zoho Mail, Yandex Mail and AOL Mail, matched by domain or by MX host. Outlook.com and Tuta are listed only to explain that they cannot be used; Proton Mail fills in Proton Mail Bridge's defaults (`127.0.0.1`, `1143` and `1025`, STARTTLS). Gmail, Yahoo, iCloud, Fastmail, Yandex and AOL ask for an app password. |
| Thunderbird Autoconfig | `https://autoconfig.<domain>/mail/config-v1.1.xml` and `https://<domain>/.well-known/autoconfig/mail/config-v1.1.xml`, with `?emailaddress=` | Served by the domain itself (cPanel, Mailcow, Mailu, Stalwart and many hosts). |
| Thunderbird ISPDB | `https://autoconfig.thunderbird.net/v1.1/<domain>` | Mozilla's central database. **Mozilla learns the domain** (never the full address). |
| DNS SRV ([RFC 6186](https://www.rfc-editor.org/rfc/rfc6186), [RFC 8314](https://www.rfc-editor.org/rfc/rfc8314)) | `_imaps`, `_imap`, `_submissions`, `_submission` | Through the system resolver. |
| MX record | the domain's MX, then the built-in list or ISPDB for the MX's domain | Finds custom domains hosted by Google Workspace, iCloud+, Fastmail and others. The MX's domain is a two-label heuristic (three under `co.uk`-style suffixes), not the public suffix list. |
| JMAP ([RFC 8620](https://www.rfc-editor.org/rfc/rfc8620)) | `https://<domain>/.well-known/jmap`, then a `_jmap._tcp` SRV record | Counts only a JMAP session, or a `401` with `WWW-Authenticate`. Added to whichever result wins. |

Microsoft Autodiscover is not used: it mostly returns Exchange settings, wants
credentials, and its fallback URLs have leaked passwords to unrelated domains.

Limits and safety:

- Every lookup request is HTTPS only (redirects too, at most 3), times out after 5
  seconds, and reads at most 64 KB. DNS queries time out after 3 seconds. The whole
  lookup gives up after 9 seconds; the wizard's **Skip** stops waiting sooner.
- Addresses whose domain is an IP address, a single label, or ends in `localhost`,
  `local`, `internal`, `test`, `invalid` or `example` are never looked up.
- Autoconfig XML is parsed without DTDs. Only servers Colecto can use are taken: TLS or
  STARTTLS, password sign-in, and for SMTP implicit TLS only on port `465`. A provider
  that offers only OAuth is reported as such instead.
- Nothing is guessed or tried with the password, and nothing is saved until the owner
  confirms the review. Unsigned DNS answers can be spoofed; showing every value for
  review is the defence, as RFC 6186 recommends.
- When the system resolver configuration cannot be read, DNS methods are skipped and
  the HTTPS methods still run. Discovery on Android is unverified.

## Connection security

Every IMAP and SMTP connection is encrypted before the password is sent; there is no
plaintext mode. Each server on the wizard's **Servers** step has a **Security** choice:

| Server | Setting | Behaviour |
| --- | --- | --- |
| IMAP | **TLS** (`imap_starttls: false`, default) | TLS from the first byte, usually port `993` |
| IMAP | **STARTTLS** (`imap_starttls: true`) | Plain connection upgraded with `STARTTLS`, usually port `143` |
| SMTP | **TLS** | Port `465` only: TLS from the first byte |
| SMTP | **STARTTLS** | Any other port (default `587`): `STARTTLS` is required |

SMTP security follows the port, so the choice and the port field change together.
Picking IMAP STARTTLS moves port `993` to `143` (and back); a custom port is kept.
If a server refuses or does not offer STARTTLS, the connection ends before any
credential is sent, so a downgrade cannot expose the password.

### Trusted certificate

By default, certificates are checked against the system's certificate authorities.
For a server with a self-signed certificate, open **Trusted certificate** in the account
dialog and choose its PEM file (`tls_certificate`, one `BEGIN CERTIFICATE` block, at
most 16 KB). Then, for both IMAP and SMTP of that account:

- only that certificate is trusted, and the system authorities are not;
- the host name entered for each server must still match the certificate;
- certificate and host name checks are never switched off.

The certificate is public, so it is stored with the account settings, not in the
credential vault. **Use system certificates** removes it.

### Proton Mail Bridge

[Proton Mail Bridge](https://proton.me/mail/bridge) (paid Proton plans) serves IMAP and
SMTP on this computer with its own self-signed certificate:

1. In Bridge, open the account's mailbox configuration and note the IMAP and SMTP
   ports, username and Bridge password. Keep both connections on STARTTLS.
2. Export Bridge's TLS certificate (advanced settings, **Export TLS certificates**);
   only `cert.pem` is needed.
3. In Colecto, add an account with your Proton address. The wizard fills in Bridge's
   defaults (`127.0.0.1`, ports `1143` and `1025`, **STARTTLS** for both); change the
   ports if Bridge shows others, then enter the Bridge username and password and
   choose `cert.pem` as the trusted certificate.

Bridge must run on the same machine as the Colecto Host: the desktop app works, while a
daemon in Docker needs Bridge in a reachable container or network. Bridge does not run
on Android. Bridge's certificate is issued to the IP address `127.0.0.1`, so enter that
rather than `localhost`. This was tested against a stand-in server on Linux, not a
real Bridge; IP-address certificates on Windows and macOS are unverified.

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

## Opening mail quickly

Opened messages come from caches before the network, and the newest Inbox mail is
downloaded ahead of time so it opens without waiting for the provider.

| Layer | Where | Bound | Notes |
| --- | --- | --- | --- |
| Content cache | `colecto.db` (`colecto-core/src/content.rs`) | 1000 messages and 256 MB per account; oldest evicted first | Complete raw messages keyed by account, provider identity and id. Reads take no account lock, so opening mail never waits behind a refresh |
| Prefetch | Host, after each background Inbox refresh (`colecto-host/src/sync.rs`) | Owner preference `prefetch: {enabled, count}`, default on and 50, at most 100 | Downloads the newest uncached Inbox messages, a few accounts at a time; never marks mail read. Skipped for accounts that are off |
| Reader cache | UI memory (`ui/src/lib/api.ts`) | 150 messages | Warmed from the Host with `GET …/messages/{id}/content?cached=true`, which answers only from the content cache and never reaches the provider |
| Folder lists | UI memory (`MailShell.svelte`) | Per account and folder | Switching Inbox, Drafts and Sent shows the last list at once, then refreshes it |

Settings → Mail turns prefetch on or off and sets how many messages to keep ready.

### Sync status

`GET /api/v1/sync` (owner only) lists each account's sync state; changes stream as `sync`
events on `/api/v1/events`. The message list footer shows it.

| Field | Values |
| --- | --- |
| `mode` | `local` (sample mail), `jmap_push`, `imap_idle`, `polling` (with `interval` seconds), `offline` (last refresh failed; `error` says why) |
| `activity` | `idle`, `syncing`, `prefetching` |
| `progress` | `[done, total]` while syncing or prefetching, such as "Downloading 12/50", otherwise `null` |

## First run and turning accounts on or off

On first launch the app opens a short onboarding screen instead of the workspace:
welcome, then the same account form as Settings → Accounts, then a confirmation that
opens the new account's Inbox. **Explore with sample mail first** skips setup and
turns the demo account on. Onboarding shows while the owner preference `onboarded`
is false and no real account exists; the built-in demo account never counts as an
added account. Finishing or skipping sets `onboarded` to true. It lives in the shell
(`ui/src/Onboarding.svelte`), not an extension, because it is bootstrapping and
handles credentials.

Every account row in Settings → Accounts has an on/off switch, stored in the owner
preference `accounts_enabled` (`{ "<account id>": bool }`, merged on update, at most
256 entries). Accounts without an entry use the default: **the demo account is off,
every other account is on.**

| | On | Off |
| --- | --- | --- |
| Account switcher and "Switch account" commands | Listed | Hidden |
| Background polling, IMAP IDLE / JMAP push, prefetch | Runs | Skipped; a running push watcher stops at its next check |
| Saved settings, credentials, cached mail, drafts | Kept | Kept |
| HTTP API, MCP and API keys | Available | Still available; the switch is a workspace choice, not an access control |

Turning off the open account switches to another account that is on. With every
account off, the mail pane says so and links to Settings → Accounts. Removing an
account also forgets its on/off choice.

## Remove an account

The owner-only `DELETE /api/v1/accounts/{id}` removes the saved account and its
credentials, cached mail, local drafts, labels and folder catalog on this device.
It does not change anything on the mail server. Unlock credentials first; the
sample account cannot be removed. The selected-account preference is cleared if
it points to this account. Removal is not exposed through MCP.

Disconnect (`POST /api/v1/accounts/{id}/disconnect`) forgets credentials but keeps
the account record and local data for reconnecting later. The current settings
Remove icon still performs this disconnect operation.

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

The vault also holds Host-owned secrets whose names start with `host:`, today the
optional [AI service keys](ai.md). They share the vault's protection: locking makes them
unavailable and **Reset saved credentials** deletes them too.

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

Removal, disconnect, vault lock/reset and account reconfiguration revoke push watcher
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

Each saved account has Reconnect, Edit and Remove icon buttons, and every account,
including the demo account, has an on/off switch (see above). Reconnect and Edit
open the same prefilled dialog; leave the credential blank to keep it. Remove asks
before disconnecting and forgetting the saved credential. The account record,
local mail and drafts remain. Protocol tags expose server hosts on hover. Connected
means the Host reports a live connection; the page does not invent a last-check time.
Credential protection and master-password recovery remain below the account list.
