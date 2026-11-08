# Colecto

A customizable email client with Rust core services and a Svelte UI. The
single-owner mail service runs inside the desktop Host or as a standalone daemon.
Browse mail, search Inbox/Sent through your provider, and save/reopen local drafts.
The daemon can receive mail while the UI is closed. This remains a prototype;
background receiving currently polls the latest 100 Inbox messages.

The client defaults to dark mode. Open the gear beside the profile at the bottom
of the sidebar for one Settings pane: appearance, accounts and credentials,
server connections, scoped API keys, and extensions. Choose System, Dark, or Light. Account profile
names are optional and fall back to the email address. Opening mail adds closable,
account-specific tabs; Inbox, Drafts, and Sent remain below the open tabs.

See [daemon deployment and notifications](docs/daemon.md) for authenticated remote
HTTPS deployment, local HTTP over Unix sockets, supervisor-independent startup,
and the optional Linux desktop notification agent. Systemd is optional.
Apple and native Android push integration are deferred; a mobile browser layout
is not evidence that native FCM/UnifiedPush or Android background services work.

## Run the local demo

Install stable Rust (1.89 or newer) with `rustfmt` and `clippy`, `just`, and
Bun 1.4.4 or newer. Linux also needs OpenSSL development libraries and pkg-config
(for example, `libssl-dev`, `libdbus-1-dev`, and `pkg-config` on Debian/Ubuntu). From the repository root:

```sh
just setup-ui
just demo
```

Open **http://127.0.0.1:4173**. Stop with Ctrl+C. The demo binds only to IPv4
loopback; open the printed address exactly, rather than `localhost`. The Host
serves the built UI and API from the same origin. The browser prompts for the
owner access token: open the private `.colecto-demo/owner-token` file locally and
copy its contents into that prompt. With `COLECTO_DATA_DIR`, use `owner-token`
inside that directory instead. The Host generates the file with Unix mode `0600`
and tightens its owned data directory to `0700`; it never prints the secret or
puts it in a URL. Do not share or commit this file. An existing non-private token
file is rejected; correct its ownership/permissions before restarting.

The native app (`just native`) connects automatically, injecting its local owner
credential only at the exact local service origin. Remote HTTPS navigation
receives no local credential. Ordinary browsers retain the entered owner token
only for that browser session. In Settings → API keys, create separately scoped
keys for automation, selecting explicit accounts and operations. Copy each key
when created; it cannot be recovered later. Scoped keys cannot manage settings
or directly send mail.

Use **Compose** to write a note, then **Save draft**. Open **Drafts** to resume it.
Leaving an edited draft asks before discarding unsaved changes. Saving failures
keep the editor text, and never report success. Without account configuration, sending is disabled. With an account configured,
**Send** submits plain-text mail through SMTP or JMAP.

The Host saves drafts to `.colecto-demo/drafts.json`, using format version 1.
This directory is ignored by git. Drafts are plain text on disk and are not
encrypted; use sample content. Only one Host may open a data directory at a time.
A corrupt or unsupported store stops startup without overwriting it. Preserve
that directory before manually attempting recovery.

Optional environment variables:

```sh
COLECTO_PORT=4180 COLECTO_DATA_DIR=/path/to/demo-data just demo
```

A relative data directory is relative to the command's working directory.
Keep this directory between runs to preserve drafts. The demo has no data-format
migration mechanism yet; a future incompatible format must provide one.

## Check changes

```sh
just verify          # Rust fmt/check/Clippy/tests, Svelte/TypeScript, UI build
just perf-check      # verification plus three recorded Linux resource runs
just setup-browser   # install the test Chromium browser once
just ui-test         # browser flows and actual Host restart persistence
```

The browser tests run a separate Host on port 4179 with temporary sample data.
They do not use your `.colecto-demo` drafts. Playwright may require system browser
libraries on some Linux installations. The tests cover a narrow viewport, but
real phone touch/keyboard behavior has not been verified.

Run `just` for all recipes. After editing UI source, `just demo` rebuilds it.

## Prototype boundaries

- `colecto-extension-api`: version 1 wire types for the operations used today.
- `colecto-core`: TLS IMAP/SMTP, prototype JMAP, MIME plain-text decoding, sample messages, draft
  validation, IDs, and atomic JSON storage.
- `colecto-host`: authenticated loopback HTTP/static adapter; validates owner sessions,
  scoped API keys, Host/origin, and built-in
  module grants scoped to an explicitly selected account, persisted extension policy,
  and isolated Wasm panels. API bodies are bounded to 512000 bytes.
- `ui/src/extensions`: replaceable bundled mailbox, reader, and composer views.
  The Host supplies their service clients; views cannot access storage directly.

Built-in module identities are routing labels for trusted application code.
Third-party Wasm panels run without WASI under resource limits, start disabled
with no grants, and emit only declarative text/actions. Third-party JavaScript
and HTML are not loaded. An optional Tauri desktop runner uses the same loopback
Host and exposes no guest native IPC. Attachment support, sync and tested mobile
builds are not implemented. See [the extension contract](docs/extensions.md) for
the exact security boundary and `just native` prerequisites.

The frontend build currently inherits a low-severity `deepmerge` advisory from
`@sveltejs/vite-plugin-svelte`, with no upstream fix. The merge runs on trusted
build configuration, not email or API input; track upstream before adding
untrusted build configuration.

See [project direction and architecture](docs/README.md), the
[approved prototype design](docs/prototype-design.md), and [agent instructions](AGENTS.md).

## Connect a mail account

IMAP receives mail; SMTP sends mail. **POP3 will not be supported.**
Use **Settings → Accounts & security** inside the app to add IMAP/SMTP or JMAP accounts, edit
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
before `just demo` (set secrets in your local environment, never in source control):

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
identity and drafts. Saved settings then take precedence; use Settings → Accounts & security to
update them. A password-protected vault suppresses environment credentials while
locked; reconnect/import through the UI after unlocking. Secrets are never
returned through the API or saved in the draft store. OAuth-only providers are not supported yet.

Opening Inbox fetches the latest 100 messages, read-only, without marking them
seen. Reopen Inbox to refresh. Messages larger than 2000000 bytes are skipped;
HTML-only messages show a placeholder. The latest successful Inbox snapshot persists in a bounded `mail-cache.json`
inside each account store. It is plain text, keyed to provider identity, and
restored on reconnect; receiving failures preserve the previous snapshot.
Preserve and remove a corrupt or unsupported cache file to rebuild it. Sent and
search results are held in a bounded reader cache in memory. Search uses upstream
IMAP TEXT or JMAP text filters and returns at most 100 matches. Sent uses the
provider role, with conventional IMAP folder-name fallbacks. SMTP acceptance does
not automatically append a Sent copy. Drafts remain local and unencrypted. New accounts have separate stores beneath
`COLECTO_DATA_DIR/accounts/<account-id>/`; the existing root draft store keeps its
legacy identity and is never assigned to a new account automatically. Disconnect
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

## Bun troubleshooting

Run repository tasks with `just`, or run UI scripts from `ui/`, for example
`cd ui && bun run --bun build`. There is no root `package.json`; `bun run build`
from the repository root fails with `Script not found "build"`.

`bun run` is not a test runner: executing the restart test that way fails with
`Cannot use test outside of the test runner`. `just ui-test` runs that test with
`bun test`, then runs Playwright separately. Playwright matches only `*.spec.ts`
to avoid collecting Bun tests. `bun run --bun test` explicitly runs the Playwright
package script; bare `bun test` invokes Bun's own runner instead.

The project uses `ui/bun.lock` and frozen installs. No npm installation is needed.
These failures were reproduced locally; without your original error output,
they do not establish which failure occurred on your machine.

## Prototype JMAP account

Instead of IMAP/SMTP, set `COLECTO_JMAP_SESSION_URL` to your provider's HTTPS
session resource (often `https://provider.example/.well-known/jmap`),
`COLECTO_JMAP_TOKEN` to a provider-issued bearer token, and
`COLECTO_MAIL_ADDRESS` to an address advertised by that account's JMAP identities.
Run `just demo`. Do not also set IMAP/SMTP variables. Partial configuration fails
startup; server authentication and capability checks happen on first use.
Tokens use the same credential storage as IMAP passwords; do not put them in source control. OAuth login and token
refresh are not implemented. Redirects are rejected; supply the final session URL.

This is an experimental transport following [RFC 8620](https://www.rfc-editor.org/rfc/rfc8620)
and [RFC 8621](https://www.rfc-editor.org/rfc/rfc8621), not a full sync client.
It uses the primary mail account, fetches the latest 100 Inbox messages without
marking them seen, displays plain text (up to 100000 bytes per body part), and
keeps drafts local. HTTP responses are limited to 8000000 bytes, with 30-second
request timeouts. HTML, attachments, changes/push, and pagination are unsupported.
The existing extension API and UI are shared by both transports.

Sending requires submission capability, a matching identity, and mailboxes with
`drafts` and `sent` roles. It creates a server draft and submits it, requesting a
move to Sent after success. Submission acceptance is not final delivery or a
guarantee that the subsequent mailbox update succeeded. Failure can leave a
server draft or an uncertain send; check Sent and Drafts before retrying. There
are no automatic retries or local draft deletions. Provider interoperability has
not been verified against a live account; use a test account and separate data
directory for this prototype.


## Optional master password

In **Settings → Accounts & security → Credential protection**, enable password protection with a
master password of at least 12 bytes. This is optional: the default is your
system credential store. Password mode also works when no desktop keychain is
available. Keep the password somewhere safe: Colecto cannot recover it.

Password protection encrypts **credentials only**, not email bodies, local drafts,
or connection metadata. The version 1 `accounts.json` registry contains settings
and an encrypted credential vault. Default mode references an OS entry holding a
fixed-size random encryption key, so long provider tokens do not exceed Windows
credential blob limits. Password mode derives its key from your master password. OS entries are
namespaced by the canonical data directory. Moving that directory requires
reconnecting accounts in system-store mode; retain a backup before moving it.

The vault uses Argon2id v19 (64 MiB, 3 iterations, 1 lane) and XChaCha20Poly1305,
with random salt/nonces and authenticated account metadata. The password and
derived key are not saved. It locks on Host restart; **Lock credentials** also
removes active mail credentials from memory. Local drafts remain accessible.
Unlocking is shared by tabs using the same local Host. A tab that changes account
security elsewhere may need to reopen Settings → Accounts & security to refresh its status.

You can change the password while unlocked or return to the system credential
store. Credential transitions journal replacement entries before switching the
registry and retry obsolete-entry cleanup after interruption. If cleanup fails,
the UI reports pending cleanup; unlock the OS store and retry. A startup cleanup
failure stops the Host with an actionable error rather than silently leaving a
protection transition incomplete. Preserve `accounts.json` on corruption; Colecto
does not overwrite unsupported or malformed files.

**Reset saved credentials** forgets credentials for all accounts without deleting
local drafts. It requires confirmation and every account must then be reconnected.
This is the recovery path for a forgotten password, not password recovery.

The implementation follows the official [keyring API](https://docs.rs/keyring/3.6.3/keyring/),
[Argon2 API](https://docs.rs/argon2/0.5.3/argon2/), and
[authenticated encryption API](https://docs.rs/chacha20poly1305/0.10.1/chacha20poly1305/).
OS-keychain behavior on macOS/Windows, live provider connections, and real phone
interaction require platform testing; automated tests use an injected credential
store or a password vault with sample connection settings.

Bundled mail UI packages have TOML manifests in `extensions/`. The **Settings → Extensions**
Host control shows metadata, enables/disables packages, and limits service
capabilities per account, saved across restarts. It can install isolated
third-party Wasm panels. `just native` starts the optional Tauri desktop runner. See
[the extension contract](docs/extensions.md) for setup and security limits.

## Resource regression checks

Run `just perf-check` after feature changes. Three isolated workloads record raw
CPU and memory samples, mean/median, and environment/build provenance under
`performance-results/`. Preserve that history for comparisons; see
[resource measurement limits](docs/performance.md). Initial comparisons report
changes without enforcing noisy hardware-dependent budgets.
