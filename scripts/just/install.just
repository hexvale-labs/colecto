set working-directory := '../..'

# Install everything a fresh checkout needs, except browsers for UI tests.
[default]
all: rust ui

# Fetch the locked Rust dependencies.
rust:
    cargo fetch --locked

# Install the locked frontend dependencies.
ui:
    cd ui && bun install --frozen-lockfile

# Install Chromium and WebKit for the browser tests (once per machine; WebKit also needs system libraries).
browsers:
    cd ui && bunx playwright install --with-deps chromium webkit

# Install the desktop app for this user (Linux): binary, menu entry and icons. Rerun to update; `release` installs an optimized build.
desktop profile='debug':
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{profile}}" in
      debug) {{just_executable()}} --justfile {{justfile()}} build native ;;
      release) {{just_executable()}} --justfile {{justfile()}} build release ;;
      *) echo "Use: just install desktop [debug|release]" >&2; exit 2 ;;
    esac
    target=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
    data="${XDG_DATA_HOME:-$HOME/.local/share}"
    bin="${XDG_BIN_HOME:-$HOME/.local/bin}/colecto"
    # Copy the binary (the UI is embedded) so later builds or `cargo clean` do not change the installed app.
    # Replace it with a rename so a running copy keeps working and launches after this get the new version.
    install -Dm755 "$target/{{profile}}/colecto" "$bin.new"
    mv -f "$bin.new" "$bin"
    # Earlier versions installed the entry as app.colecto.mail; remove it so the menu shows one Colecto.
    rm -f "$data/applications/app.colecto.mail.desktop" "$data"/icons/hicolor/*/apps/app.colecto.mail.{png,svg}
    for size in 32x32 128x128 256x256; do
      file=crates/colecto-app/icons/$size.png; [ "$size" = 256x256 ] && file=crates/colecto-app/icons/128x128@2x.png
      install -Dm644 "$file" "$data/icons/hicolor/$size/apps/colecto.png"
    done
    install -Dm644 crates/colecto-app/icons/icon.svg "$data/icons/hicolor/scalable/apps/colecto.svg"
    mkdir -p "$data/applications"
    sed "s|^Exec=.*|Exec=$bin|" packaging/linux/Colecto.desktop > "$data/applications/Colecto.desktop"
    command -v update-desktop-database >/dev/null && update-desktop-database -q "$data/applications" || true
    echo "Installed Colecto ({{profile}}) as $bin with a desktop entry in $data/applications"
