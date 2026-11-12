set working-directory := '../..'

# Run every cleanup recipe (for now only `desktop`).
[default]
all: desktop

# Undo `just install desktop`: remove the installed binary, menu entry and icons. Mail data and settings are kept.
desktop:
    #!/usr/bin/env bash
    set -euo pipefail
    data="${XDG_DATA_HOME:-$HOME/.local/share}"
    bin="${XDG_BIN_HOME:-$HOME/.local/bin}/colecto"
    rm -f "$bin" "$bin.new" "$data/applications/Colecto.desktop" "$data/applications/app.colecto.mail.desktop" \
      "$data"/icons/hicolor/*/apps/colecto.{png,svg} "$data"/icons/hicolor/*/apps/app.colecto.mail.{png,svg}
    command -v update-desktop-database >/dev/null && update-desktop-database -q "$data/applications" || true
    echo "Removed the Colecto desktop install from $data and $bin"
