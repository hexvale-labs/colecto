# Recipes are grouped into modules: `just build rust`, `just test app`, `just run web`.
# A module name on its own runs its default recipe, so `just check` runs all static checks.

mod install 'just/install.just'
mod build 'just/build.just'
mod check 'just/check.just'
mod test 'just/test.just'
mod run 'just/run.just'

# List every recipe, including those inside modules.
[default]
help:
    @{{just_executable()}} --justfile {{justfile()}} --list --list-submodules

# Format Rust and Dart source files.
fmt:
    cargo fmt --all
    cd app && dart format .

# Run the routine verification suite (Rust + Flutter).
verify: check::fmt check::rust check::lint test::rust check::app test::app

# Set the release version in Cargo.toml, Cargo.lock and app/pubspec.yaml, for example `just bump-version 0.10.0`.
bump-version version:
    #!/usr/bin/env bash
    set -euo pipefail
    [[ "{{version}}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "expected X.Y.Z, got {{version}}" >&2; exit 1; }
    sed -i -E '/^\[workspace\.package\]/,/^\[/ s/^version = "[^"]+"/version = "{{version}}"/' Cargo.toml
    # Build number = major*1000000 + minor*1000 + patch, the Android versionCode v0.9.0 used,
    # so every release installs over the one before it.
    IFS=. read -r major minor patch <<< "{{version}}"
    sed -i -E "s/^version: .*/version: {{version}}+$((major * 1000000 + minor * 1000 + patch))/" app/pubspec.yaml
    cargo update --workspace --offline --quiet
    grep -q '^version = "{{version}}"' Cargo.toml && grep -q '^version: {{version}}+' app/pubspec.yaml
    echo "Version set to {{version}}"
