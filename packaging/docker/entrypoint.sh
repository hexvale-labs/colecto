#!/bin/sh
# Create the remote access token on first start, then replace this shell with the daemon so it receives signals directly.
set -eu
if [ -n "${COLECTO_TOKEN_FILE:-}" ] && [ ! -e "$COLECTO_TOKEN_FILE" ]; then
    (umask 077 && od -An -N32 -tx1 /dev/urandom | tr -d ' \n' > "$COLECTO_TOKEN_FILE")
    echo "Generated a new access token in $COLECTO_TOKEN_FILE" >&2
fi
exec "$@"
