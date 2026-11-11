# Arch User Repository packages

| Package | Builds from | Installs |
| --- | --- | --- |
| `colecto` | the release's source tarball | `colecto`, `colecto-daemon`, `colecto-mcp`, desktop entry, icons, systemd user units |
| `colecto-bin` | the release's `.deb` | the desktop app only |
| `colecto-git` | the latest commit on `main` | the same files as `colecto` |

The files here are templates: on every `vX.Y.Z` tag, the release workflow's `aur` job runs
`scripts/python/aur_publish.py` in an Arch container. It sets `pkgver`, resets `pkgrel` to 1,
fills in checksums with `updpkgsums`, regenerates `.SRCINFO` and pushes each package that
changed. `colecto-git` derives its version itself, so it is only pushed when its PKGBUILD
changes. The job is skipped while the `AUR_SSH_PRIVATE_KEY` repository secret is missing.

To publish by hand from an Arch machine or container, run as root:

```sh
AUR_KEY="$(cat ~/.ssh/aur)" python3 scripts/python/aur_publish.py --version 0.1.0 --push
```

Without `--push`, the script only renders the packages into `--workdir` (default `/tmp/aur`),
which is useful for `makepkg -si` and `namcap` checks.
