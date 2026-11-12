# Self-hosting the daemon

`colecto-daemon` is the mail service on its own: it keeps your accounts, receives mail
while no app is open, and serves the web UI and API. Run it in Docker (quickest) or
under any supervisor, then point the browser or the Colecto apps at it.

## Docker

The daemon keeps your accounts and receives mail while no app is open. Run it on a
server, then reach it from the browser or the Colecto app. Images for `amd64` and
`arm64` are published to `ghcr.io/hexvale-labs/colectod` for every release
(tags `latest`, `vX.Y.Z`, `vX.Y` and `vX`).

The daemon speaks plain HTTP and accepts outside connections only behind HTTPS, so
the example below puts [Caddy](https://caddyserver.com) in front of it. Caddy gets a
TLS certificate automatically.

### Start it

You need a domain name pointing at the server, with ports 80 and 443 open.

```sh
mkdir colecto && cd colecto
curl -fsSLO https://raw.githubusercontent.com/hexvale-labs/colecto/main/packaging/docker/compose.yaml
echo "COLECTO_DOMAIN=mail.example.org" > .env
docker compose up -d
docker compose exec colecto cat /data/access-token
```

On first start the container creates a random access token in its data volume. That
last command prints it; keep it somewhere safe, like a password manager.

### Connect to it

- **Browser:** open `https://mail.example.org` and paste the access token.
- **Colecto app:** open **Settings → Servers**, enter `https://mail.example.org`,
  then paste the access token.
- **Desktop notifications on Linux:** run `colecto-daemon notify` with
  `COLECTO_PUBLIC_ORIGIN` and `COLECTO_TOKEN_FILE`; see [remote HTTPS deployment](#remote-https-deployment).

Then add your mail accounts in **Settings → Accounts**.

### Credentials in a container

A container has no desktop keychain, so turn on **master password protection** in
**Settings → Accounts** before adding accounts. Credentials are then encrypted with a
key derived from that password. After the container restarts, the vault stays
locked, and mail is not fetched, until you unlock it once in Settings.

### Configuration

Set these under `environment:` in `compose.yaml`.

| Variable | Default in the image | Meaning |
| --- | --- | --- |
| `COLECTO_PUBLIC_ORIGIN` | required | The public HTTPS address, such as `https://mail.example.org` |
| `COLECTO_TOKEN_FILE` | `/data/access-token` | Access token file; created on first start if missing |
| `COLECTO_DATA_DIR` | `/data` | Database, owner token and extension data |
| `COLECTO_SYNC_SECONDS` | `60` | How often to poll for mail (at least 10); push still applies |
| `COLECTO_LOG` | `info` | Log filter, such as `debug` or `warn,colecto_core=debug` |
| `COLECTO_BIND`, `COLECTO_PORT` | `0.0.0.0`, `4173` | Listener inside the container |

Keep the `colecto` service off the public network; only Caddy should reach port 4173.
To use another reverse proxy, forward to `colecto:4173`, keep the original `Host`
header, and do not buffer `/api/v1/events` (it is a server-sent event stream).

### Logs, updates and backups

```sh
docker compose logs -f colecto           # follow the logs
docker compose pull && docker compose up -d  # update to the latest release
```

Everything the daemon stores lives in the `colecto-data` volume. Back it up while the
container is stopped, for example:

```sh
docker compose stop colecto
docker run --rm -v colecto_colecto-data:/data -v "$PWD":/backup debian:bookworm-slim \
  tar czf /backup/colecto-data.tgz -C /data .
docker compose start colecto
```

To rotate the access token, delete `/data/access-token` and restart the container; a
new one is created and the old one stops working.

### Build the image yourself

From a checkout:

```sh
docker build -t colecto-daemon .
```

Or replace `image:` with the commented `build:` lines in `compose.yaml`.

## How the service runs

Colecto has one reusable Rust `MailService`. The `colecto-host` package runs it as
`colecto-host` (the local browser demo) and `colecto-daemon` (foreground service
operation); the `colecto-app` package runs it inside the desktop and mobile app. The daemon and the app use the same
account stores and extension policy. There is one owner per data directory,
with multiple accounts and clients. Multi-user hosting and admin users are not
implemented.

### Local foreground service

Use `just run daemon` to build and run the service and UI. For a supervisor, build
with `just build rust`, then run the executable directly:

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

### Remote HTTPS deployment

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
request requires an explicit owner or scoped API-key bearer before operation grants
are checked. Never put the token in a URL. Serve the UI from this same daemon
origin; its login/settings interface keeps the token in session storage.

The downstream API remains Colecto's versioned HTTP API, not a complete JMAP
server. Upstream JMAP/IMAP/SMTP support remains separate from this choice.

### HTTP over a Unix socket

```sh
install -d -m 700 "$HOME/.local/run/colecto"
COLECTO_SOCKET="$HOME/.local/run/colecto/mail.sock" \
COLECTO_DATA_DIR="$HOME/.local/state/colecto" \
./target/debug/colecto-daemon
auth_file=$(mktemp)
printf 'Authorization: Bearer %s\n' "$(cat "$HOME/.local/state/colecto/owner-token")" > "$auth_file"
curl --unix-socket "$HOME/.local/run/colecto/mail.sock" \
  -H "@$auth_file" -X POST -H 'Origin: http://localhost' \
  -H 'Host: localhost' http://localhost/api/v1/session
curl --unix-socket "$HOME/.local/run/colecto/mail.sock" \
  -H "@$auth_file" -H 'Host: localhost' -H 'x-colecto-extension: host' \
  -H 'x-colecto-account: demo' http://localhost/api/v1/accounts
rm "$auth_file"
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

### Logs

The daemon logs to stderr through `tracing`. `COLECTO_LOG` (or `RUST_LOG`) sets the
filter, for example `COLECTO_LOG=debug` or `COLECTO_LOG=warn,colecto_core=debug`; the
default is `info` for Colecto and `warn` for libraries. Startup prints the listening
address on stdout so scripts can read it.

Under systemd (detected through `JOURNAL_STREAM`) every line starts with a syslog
priority (`<3>` error, `<4>` warning, `<6>` info, `<7>` debug) and has no timestamp of
its own, so the journal records real levels:

```sh
journalctl --user -u colecto-daemon -f          # follow
journalctl --user -u colecto-daemon -p warning  # warnings and errors only
```

Elsewhere, including Docker, lines carry an RFC 3339 timestamp; colours are used
only when stderr is a terminal. HTTP requests are logged at `debug` with method and
path only. Credentials, tokens and message contents are never logged.

### Supervisors, including systems without systemd

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

### Desktop notifications from a remote daemon

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
headers and an explicit owner bearer. Cookie-only streaming is rejected; use
a streaming HTTP client that can attach Authorization, not a cookie-dependent
browser EventSource or a token in the URL. It starts with `ready`, then emits `mail` with `{account, ids}` for
newly observed inbox IDs, `approval` with `{id, account, from, subject}` when automation
requests a send, `navigate` with `{target, account}` from MCP `app.navigate`, and `resync`
if a slow subscriber misses events. Mail events follow polling and, where the provider
supports it, IMAP IDLE or JMAP push, so new mail usually appears within seconds. The
web client subscribes with `fetch` and shows these as in-app notifications. The
stream is bounded and has no historical replay. Desktop notifications may be
missed during disconnection; opening the client fetches the current mail state.
The agent deliberately ignores initial `ready` and does not notify for old mail.

### Owner bearers and scoped API keys

Every API call requires an explicit `Authorization: Bearer <token>` header with
the owner bearer or a scoped API key. Cookies never establish owner authority,
in local or remote mode. The bundled UI keeps the owner bearer in sessionStorage,
which belongs to its exact origin (scheme, hostname, and port), and explicitly
sends it on each request. Another loopback service does not receive it through
automatic browser cookie behavior.

`POST /api/v1/session` validates an owner bearer with exact configured `Host` and
`Origin`; it issues no cookie, session, or additional credential. Headers alone
cannot authenticate an OS user. Local startup generates `owner-token` in the data
directory with mode `0600`. It tightens an existing data directory to `0700` only
after verifying it belongs to the current OS user; a foreign-owned directory
fails with an actionable error. Read that private file to connect an ordinary
browser. Its contents are never printed or put in the URL. The native shell
injects it only at its exact local origin; HTTPS server navigation receives no
local credential. Remote deployments continue to use `COLECTO_TOKEN_FILE` instead
of a generated local token. Restart after owner token rotation.

Settings → API keys generates named keys for explicitly selected accounts and
capabilities, with optional expiry. Copy the secret when created: list responses
contain metadata only. Revocation and expiry are checked on each request. New
accounts receive no existing key grants. A supplied invalid or scoped bearer
never falls back to a privileged cookie or local bootstrap. Scoped keys cannot
manage accounts, credentials, extensions, other keys, events, or owner-token validation.
Direct delivery is denied for scoped callers, including keys with `messages.send`;
reviewed send requests use the approval flow described below.

Automation uses `Authorization: Bearer <key>` plus `x-colecto-account`.
`x-colecto-extension` cannot confer scoped privileges. Retain the configured
`Host`; mutation requests also require the configured `Origin`. For owner Unix
socket calls, send the private owner bearer explicitly on every request, together
with `Host: localhost` and `Origin: http://localhost` for mutations. The
`/api/v1/session` validation step is optional and confers no ongoing authority.

`api-keys.json` is a versioned private policy file: random public IDs and SHA-256
verifiers, never full secrets. Secrets contain 256 random bits independently of
the ID. Atomic writes use private files, fsync, and rename. Corrupt, unsupported,
or non-private policy fails closed; missing policy authenticates no keys.
Policy permits at most 128 retained keys, 80 bytes per name, 64 explicit accounts
per key, and eight existing capabilities. Revoked metadata is retained. Last-used
metadata is updated at most once per minute per key; this is approximate activity
metadata, not an audit log. Keep the data directory private and owned by its OS
user. Windows filesystem ACL validation remains outside the tested Unix path.

Configured static assets must not contain the private data directory; startup rejects overlapping roots so private owner credentials cannot become public assets.

### Scoped automation and MCP

Generate a named API key in Settings → API keys, selecting each account and
operation explicitly. `messages.send` grants permission to **request** delivery.
It never permits an API caller or MCP client to approve or deliver mail directly.
Review requests in Settings → Send approvals. The owner composer still sends
when the owner presses Send. Routing headers cannot turn a scoped key into an
owner.

MCP is **off by default**. Turn it on in Settings → API keys (it is stored with the
owner preferences); until then `/api/v1/mcp` answers 404 and keys still work over
HTTP. The shared local Host and daemon expose `/api/v1/mcp` using the official Rust
MCP SDK (`rmcp` 3.9.0), stateless Streamable HTTP JSON responses and the stable
`2025-11-25` protocol. Configure `Authorization: Bearer <scoped-key>` in your
client. There is no OAuth authorization server, discovery or dynamic client
registration; clients that require OAuth discovery are not supported. Host
validation remains required; a supplied browser Origin must match the service.
Nonbrowser MCP requests may omit Origin because every request requires an
explicit bearer. All calls recheck current key policy, including revocation and
expiry after initialization.

For stdio clients, build `colecto-mcp` with `just build rust`, then configure the
executable with one argument, such as `https://mail.example.com/api/v1/mcp`
(or `http://127.0.0.1:4173/api/v1/mcp` locally). Set `COLECTO_MCP_KEY_FILE` to a
private `0600` file containing a scoped key. `COLECTO_MCP_KEY` is an alternative
environment variable; do not put secrets in arguments or URLs. The broker
connects to the already running Host rather than opening a separate mail store.
Only MCP JSON goes to stdout. Errors go to stderr without credential contents.
Input lines are limited to 512000 bytes and responses to 1000000 bytes; oversized
messages fail rather than truncate. Redirects are disabled.

Tools expose `account.read`, `messages.list`, `messages.read`, `drafts.list`,
`drafts.read`, `drafts.write`, `labels.list`, `messages.label`, `messages.send`, `send.status`
and `app.navigate`. Each takes `account`; reads/status also take `id`, writes/send take
`draft`, label updates take `label`, and navigation takes `target`. Message lists accept
`folder` and `q`; `q` searches the provider by sender, subject and text.

`app.navigate` needs the `app.navigate` capability, which only API keys can hold. It
opens one of `NAVIGATION_TARGETS` (`inbox`, `drafts`, `sent`, `compose` or
`settings/<section>`) in every connected Colecto window by broadcasting a `navigate`
event; mail pages also switch those windows to the key's `account`. The result reports
how many windows were listening. Discovery
confers no grants. Invalid inputs and operation failures return structured tool
errors; authentication failures use HTTP 401. No approval tool exists.

The HTTP equivalent is `POST /api/v1/send` with an explicit scoped bearer,
`x-colecto-account`, matching `Origin` and a `DraftInput` JSON body. A successful
request returns `{ "id": "…", "status": "pending", "expires_at": … }`, which
means **waiting for human approval**, never sent. Poll
`GET /api/v1/send-requests/{id}` or MCP `send.status` at most once per second.
Only the requesting key can inspect its statuses; only the owner can list full
snapshots or approve/reject. Requests expire in 10 minutes; at most 16 pending
requests per key and 128 retained requests per installation are allowed.
Snapshots are at most 110000 serialized bytes in this slice. Retained snapshots
are purged after 24 hours on journal access/restart or the existing receive maintenance pass; they are local plaintext in
the private `colecto.db` send-approval journal. Do not store the data directory under static
assets. Edits require a new request.

Approval rechecks the requesting key, account connection and From identity,
then persists `submitting` before initiating transport once. Replaying approval
cannot submit again. Transport failures become `uncertain`; on restart, an
interrupted `submitting` also becomes `uncertain`. Never automatically retry an
uncertain request: check Sent or contact the recipient. This provides at-most-once
application initiation, not exactly-once SMTP delivery. A disconnected reviewing
browser does not cancel an already initiated send. Reject, expiry, revoked keys,
removed/disconnected accounts or missing grants prevent initiation.
