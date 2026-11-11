# Recipes are grouped into modules: `just build ui` (or `just build::ui`), `just test rust`, `just install all`.
# A module name on its own runs its `all` recipe, so `just build` builds everything.

mod install 'just/install.just'
mod build 'just/build.just'
mod check 'just/check.just'
mod test 'just/test.just'
mod run 'just/run.just'

# List every recipe, including those inside modules.
[default]
help:
    @{{just_executable()}} --justfile {{justfile()}} --list --list-submodules

# Format Rust source.
fmt:
    cargo fmt --all

# Run the routine verification suite.
verify: check::fmt check::rust check::lint test::rust check::ui test::ui build::ui

# Measure three isolated daemon/browser workloads and retain timestamped history.
perf: build::rust build::ui test::perf
    python3 scripts/resource_bench.py

# Run the normal verification suite and record resource usage for feature changes.
perf-check: verify perf

# Android and iOS icons are generated into their projects by the release workflow.
# Regenerate the desktop app icons from the single source, crates/colecto-app/icons/icon.svg.
icons:
    #!/usr/bin/env bash
    set -euo pipefail
    out=$(mktemp -d); trap 'rm -rf "$out"' EXIT
    bunx --bun @tauri-apps/cli@2.9.4 icon crates/colecto-app/icons/icon.svg -o "$out" > /dev/null
    cp "$out"/{32x32.png,128x128.png,128x128@2x.png,icon.png,icon.ico,icon.icns} crates/colecto-app/icons/

# Install the Linux desktop entry and icons. Pass `release` to point it at the release build.
desktop-install profile='debug':
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{profile}}" in
      debug) {{just_executable()}} --justfile {{justfile()}} build native ;;
      release) {{just_executable()}} --justfile {{justfile()}} build release ;;
      *) echo "Use: just desktop-install [debug|release]" >&2; exit 2 ;;
    esac
    target=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
    data="${XDG_DATA_HOME:-$HOME/.local/share}"
    for size in 32x32 128x128 256x256; do
      file=crates/colecto-app/icons/$size.png; [ "$size" = 256x256 ] && file=crates/colecto-app/icons/128x128@2x.png
      install -Dm644 "$file" "$data/icons/hicolor/$size/apps/app.colecto.mail.png"
    done
    install -Dm644 crates/colecto-app/icons/icon.svg "$data/icons/hicolor/scalable/apps/app.colecto.mail.svg"
    mkdir -p "$data/applications"
    sed "s|^Exec=.*|Exec=$target/{{profile}}/colecto|" packaging/linux/app.colecto.mail.desktop > "$data/applications/app.colecto.mail.desktop"
    echo "Installed Colecto ({{profile}}) desktop entry in $data/applications"
