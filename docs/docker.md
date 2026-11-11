# Hosting the daemon with Docker

The daemon keeps your accounts and receives mail while no app is open. Run it on a
server, then reach it from the browser or the Colecto app. Images for `amd64` and
`arm64` are published to `ghcr.io/hexvale-labs/colecto-daemon` for every release
(`latest` and the release tag, such as `v1.2.3`).

The daemon speaks plain HTTP and accepts outside connections only behind HTTPS, so
the example below puts [Caddy](https://caddyserver.com) in front of it. Caddy gets a
TLS certificate automatically.

## Start it

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

## Connect to it

- **Browser:** open `https://mail.example.org` and paste the access token.
- **Colecto app:** open **Settings → Servers**, enter `https://mail.example.org`,
  then paste the access token.
- **Desktop notifications on Linux:** run `colecto-daemon notify` with
  `COLECTO_PUBLIC_ORIGIN` and `COLECTO_TOKEN_FILE`; see [daemon](daemon.md).

Then add your mail accounts in **Settings → Accounts**.

## Credentials in a container

A container has no desktop keychain, so turn on **master password protection** in
**Settings → Accounts** before adding accounts. Credentials are then encrypted with a
key derived from that password. After the container restarts, the vault stays
locked, and mail is not fetched, until you unlock it once in Settings.

## Configuration

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

## Logs, updates and backups

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

## Build the image yourself

From a checkout:

```sh
docker build -t colecto-daemon .
```

Or replace `image:` with the commented `build:` lines in `compose.yaml`.
