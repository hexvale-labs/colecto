# Mail service and deployment

Colecto has one reusable Rust `MailService` and two entrypoints in the
`colecto-host` package: `colecto-host` for the optional desktop shell/local demo,
and `colecto-daemon` for foreground service operation. Both use the existing
core account stores and extension grants. There is one owner per data directory,
with multiple accounts and clients. Multi-user hosting and admin users are not
implemented.

## Local foreground service

Use `just daemon` to build and run the service and UI. For a supervisor, build
with `just build`, then run the executable directly:

```sh
COLECTO_DATA_DIR="$HOME/.local/state/colecto" ./target/debug/colecto-daemon
```

The daemon defaults to `127.0.0.1:4173`. `COLECTO_PORT=0` chooses an available
port. `COLECTO_BIND` changes the bind IP; non-loopback listeners require remote
authentication configuration. `COLECTO_ASSETS` optionally points to a built
`ui/dist` directory; the service can run without frontend assets. All API
operations use the existing `/api/v1` contract and explicit account/module
headers. Those headers are capability routing identifiers, not authentication.

Receiving runs independently of the UI every `COLECTO_SYNC_SECONDS` seconds
(default 60, minimum 10). At most two background account operations run at once;
an account already doing another operation is skipped until a later pass. Failed
receives do not replace the last successful inbox. Polling uses the existing
bounded receive implementation (latest 100 inbox messages, not a complete mailbox mirror); persistent IMAP IDLE and incremental JMAP
synchronization are not implemented yet. Provider network timeouts still apply.

The first successful background receive establishes a notification baseline;
existing messages are not announced as new at startup. Later new IDs generate
mail events. The notification baseline is independent of UI fetches, so opening Inbox does
not consume or suppress the next background notification. This is notification best effort,
not a durable event queue or guaranteed delivery.

SIGINT and SIGTERM stop the HTTP listener and polling loop. Active provider work
finishes before shutdown; a supervisor may enforce its own final timeout. A
locked password vault stays locked after restart until the owner unlocks it
through the UI. Background service operation therefore needs an available
system credential store, or manual unlocking after startup. No plaintext
credential fallback is added.

## Remote HTTPS deployment

Run behind an HTTPS reverse proxy. The public origin must contain only the
HTTPS scheme and authority, with no path, query, or credentials. Generate an
access token in a private file:

```sh
install -d -m 700 "$HOME/.config/colecto"
(umask 077; openssl rand -hex 32 > "$HOME/.config/colecto/access-token")
COLECTO_PUBLIC_ORIGIN=https://mail.example.org \
COLECTO_TOKEN_FILE="$HOME/.config/colecto/access-token" \
COLECTO_DATA_DIR="$HOME/.local/state/colecto" \
./target/debug/colecto-daemon
```

The token must be 64–512 ASCII letters, digits, hyphens or underscores; generate
it randomly rather than selecting a password. The file must be a regular file
and, on Unix, have no group/other permission bits (`chmod 600`). The token is
never printed. Restart after token rotation to revoke the old token.

The proxy must preserve the public `Host` header, forward requests to the
loopback listener, and disable buffering for `/api/v1/events`. Keep the backend
private: it speaks HTTP, not TLS. Do not expose its port to the Internet. The
configured public origin controls origin validation; forwarded headers never
select the trusted origin. Cross-origin browser clients are not enabled.

The native shell permits navigation to HTTPS server origins selected in settings;
remote pages receive no native IPC grants. Static UI assets are public so the login interface can load. Every `/api/`
request requires `Authorization: Bearer <token>` before module/account grants
are checked. Never put the token in a URL. Serve the UI from this same daemon
origin; its login/settings interface keeps the token in session storage.

The downstream API remains Colecto's versioned HTTP API, not a complete JMAP
server. Upstream JMAP/IMAP/SMTP support remains separate from this choice.

## HTTP over a Unix socket

```sh
install -d -m 700 "$HOME/.local/run/colecto"
COLECTO_SOCKET="$HOME/.local/run/colecto/mail.sock" \
COLECTO_DATA_DIR="$HOME/.local/state/colecto" \
./target/debug/colecto-daemon
curl --unix-socket "$HOME/.local/run/colecto/mail.sock" \
  -H 'Host: localhost' -H 'x-colecto-extension: host' \
  -H 'x-colecto-account: demo' http://localhost/api/v1/accounts
```

Unix socket mode is local-only and requires an existing private parent directory
(`chmod 700`). The socket is `0600`; local access is governed by filesystem
permissions. Mutation requests must use `Origin: http://localhost` alongside the
usual account/module headers. Browsers do not directly open Unix sockets; a
native adapter or local proxy is needed for that client transport.

Startup refuses an existing socket or file and never removes it automatically.
Normal shutdown removes only the socket inode created by this instance. After a
crash or SIGKILL, stop the old process and inspect/remove its stale socket before
restarting. This intentional refusal prevents deleting another service's socket.
`COLECTO_SOCKET` and `COLECTO_PUBLIC_ORIGIN` cannot be combined.

## Supervisors, including systems without systemd

The daemon does not daemonize itself and does not call systemd APIs. Any
supervisor that launches a foreground process and sends termination signals can
run it: OpenRC, runit, s6, a container supervisor, or a terminal.

`packaging/systemd/colecto-daemon.service` is an optional **user** unit. Install
the executable at `/usr/local/bin/colecto-daemon`, put environment settings in
`~/.config/colecto/daemon.env`, copy the unit to `~/.config/systemd/user/`, then
run `systemctl --user enable --now colecto-daemon`. User-service persistence past
logout depends on the machine's user-session/lingering configuration. Nothing
requires root execution. For Unix socket mode, create its private directory
before startup or add the supervisor's corresponding runtime-directory setup.

For runit/s6, the supervised run script is simply:

```sh
#!/bin/sh
export COLECTO_DATA_DIR="$HOME/.local/state/colecto"
exec /usr/local/bin/colecto-daemon
```

Set the same environment and executable in an OpenRC service or another init
system. The supervisor owns restart policy; never launch two instances against
one data directory.

## Desktop notifications from a remote daemon

The optional Linux desktop session agent connects to the remote event stream;
it does not establish additional upstream provider connections or store mail
credentials:

```sh
COLECTO_PUBLIC_ORIGIN=https://mail.example.org \
COLECTO_TOKEN_FILE="$HOME/.config/colecto/access-token" \
./target/debug/colecto-daemon notify
```

It requires `notify-send` and the logged-in desktop notification service. Run
it in the user's graphical session, not as a root system service. It reconnects
with capped exponential backoff, reloads the token file on reconnect, and shows
generic notifications without sender, subject or body. The optional
`colecto-notifications.service` user unit reads
`~/.config/colecto/notifications.env` and follows `graphical-session.target`.
That file sets the public origin and absolute token-file path.

`GET /api/v1/events` is an authenticated SSE stream requiring Host capability
headers. It starts with `ready`, then emits `mail` with `{account, ids}` for
newly observed inbox IDs, and `resync` if a slow subscriber misses events. The
stream is bounded and has no historical replay. Desktop notifications may be
missed during disconnection; opening the client fetches the current mail state.
The agent deliberately ignores initial `ready` and does not notify for old mail.

## Android and future work

Android integration is a separate platform adapter: an app-embedded Rust service
must obey Android lifecycle and background restrictions. Local Android accounts
cannot promise uninterrupted execution. Android FCM and UnifiedPush are the
approved push choices; a server event stream alone is not either push service.
This daemon foundation does not implement device registration, FCM credentials,
UnifiedPush endpoint registration, Android Java/Kotlin services, or native mobile
build validation. Apple support is deferred. No iOS background behavior is
claimed.

## Verification record

- Added API tests for missing/wrong bearer tokens, forged Host identities,
  authenticated access, origin rejection, and invalid remote configuration.
- Added Unix socket tests covering unrelated files, active sockets,
  replacement-file preservation, and owned cleanup.
- Added notification parser tests for ready/empty/malformed messages and bounded
  input, plus stopped background-worker lifecycle coverage.
- Live provider synchronization, real reverse-proxy TLS deployment, graphical
  notification delivery, and Android hardware remain platform integration checks.

Implementation progress: the reusable service/runtime, standalone binary,
remote authentication, Unix transport, polling events, Linux notification agent,
and sample supervisor units are implemented. The standalone Unix HTTP process
has been tested through a real request and graceful SIGTERM cleanup. Remote
origin canonicalization has regression coverage for mixed-case hostnames and
explicit default HTTPS ports. Final workspace verification is recorded in the
root implementation plan, not implied by this subsystem record.

Subsystem checks passed: `cargo test -p colecto-host --all-targets --locked`
(11 unit tests, 5 daemon boundary tests, 6 existing HTTP integration tests,
1 real Unix-socket process/shutdown test), and
`cargo clippy -p colecto-host --all-targets --locked -- -D warnings`.
The process test keeps an SSE subscription open during SIGTERM and verifies
that both the stream and process exit before the owned socket disappears.
The existing upstream `imap-proto` future-compatibility warning remains.
