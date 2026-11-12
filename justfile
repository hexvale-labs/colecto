# Recipes are grouped into modules: `just build ui` (or `just build::ui`), `just test rust`, `just install all`.
# A module name on its own runs its `all` recipe, so `just build` builds everything.

mod install 'scripts/just/install.just'
mod cleanup 'scripts/just/cleanup.just'
mod build 'scripts/just/build.just'
mod check 'scripts/just/check.just'
mod test 'scripts/just/test.just'
mod run 'scripts/just/run.just'

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
    python3 scripts/python/resource_bench.py

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
