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

## GitHub setup

Create an [AUR account](https://aur.archlinux.org/register) and generate a dedicated
SSH key on your own machine:

```sh
ssh-keygen -t ed25519 -f ~/.ssh/colecto-aur -N '' -C colecto-aur-ci
```

Paste `~/.ssh/colecto-aur.pub` into your AUR account's **SSH Public Key** field.
In GitHub, open **Settings → Secrets and variables → Actions** and create
`AUR_SSH_PRIVATE_KEY` containing the entire private `~/.ssh/colecto-aur` file,
including its BEGIN and END lines. Keep that private key out of Git.
Your AUR account must own or co-maintain any existing packages with these names.

GHCR publishing uses the workflow's `GITHUB_TOKEN`; no additional token is needed.
After the first image publishes, set the organization's `colectod` package visibility
to public if anonymous downloads are wanted.

To publish by hand from an Arch machine or container, run as root:

```sh
AUR_KEY="$(cat ~/.ssh/aur)" python3 scripts/python/aur_publish.py --version 0.1.0 --push
```

Without `--push`, the script only renders the packages into `--workdir` (default `/tmp/aur`),
which is useful for `makepkg -si` and `namcap` checks.
