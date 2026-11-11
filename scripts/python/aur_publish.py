#!/usr/bin/env python3
"""Render the AUR packages in packaging/aur for a release and optionally push them.

Run as root inside an Arch Linux container (CI does this). makepkg refuses to run as
root, so checksum and .SRCINFO generation run as an unprivileged `builder` user.
With --push, AUR_KEY must hold the private SSH key of the AUR maintainer account.
"""
import argparse
import os
import pwd
import re
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PACKAGES = ['colecto', 'colecto-bin', 'colecto-git']
# The AUR's published Ed25519 host key (SHA256:RFzBCUItH9LZS0cKB5UE6ceAYhBD5C8GeOBip8Z11+4).
AUR_HOST_KEY = 'aur.archlinux.org ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIEuBKrPzbawxA/k2g6NcyV5jmqwJ2s+zpgZGZ7tpLIcN'
VERSIONED = {'colecto', 'colecto-bin'}


def run(*args, cwd=None, user=None, capture=False):
    if user:
        args = ('runuser', '-u', user, '--', *args)
    result = subprocess.run(args, cwd=cwd, check=True, text=True, capture_output=capture)
    return result.stdout if capture else None


def set_version(pkgbuild: Path, version: str):
    text = pkgbuild.read_text()
    text = re.sub(r'^pkgver=.*$', f'pkgver={version}', text, count=1, flags=re.M)
    text = re.sub(r'^pkgrel=.*$', 'pkgrel=1', text, count=1, flags=re.M)
    pkgbuild.write_text(text)


def setup_ssh(key: str) -> dict:
    ssh = Path.home() / '.ssh'
    ssh.mkdir(mode=0o700, exist_ok=True)
    identity = ssh / 'aur'
    identity.write_text(key.strip() + '\n')
    identity.chmod(0o600)
    (ssh / 'known_hosts').write_text(AUR_HOST_KEY + '\n')
    command = f'ssh -i {identity} -o IdentitiesOnly=yes -o StrictHostKeyChecking=yes -o UserKnownHostsFile={ssh}/known_hosts'
    return {**os.environ, 'GIT_SSH_COMMAND': command}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--version', required=True, help='release version without the leading v')
    parser.add_argument('--push', action='store_true', help='commit and push to the AUR')
    parser.add_argument('--workdir', type=Path, default=Path('/tmp/aur'))
    parser.add_argument('packages', nargs='*', default=PACKAGES)
    args = parser.parse_args()

    run('pacman', '-Syu', '--noconfirm', '--needed', 'git', 'openssh', 'pacman-contrib')
    try:
        pwd.getpwnam('builder')
    except KeyError:
        run('useradd', '--create-home', 'builder')
    env = setup_ssh(os.environ['AUR_KEY']) if args.push else os.environ
    shutil.rmtree(args.workdir, ignore_errors=True)
    args.workdir.mkdir(parents=True)

    for name in args.packages:
        repo = args.workdir / name
        if args.push:
            subprocess.run(['git', 'clone', f'ssh://aur@aur.archlinux.org/{name}.git', str(repo)], check=True, env=env)
        else:
            repo.mkdir()
        shutil.copy(ROOT / 'packaging/aur' / name / 'PKGBUILD', repo / 'PKGBUILD')
        if name in VERSIONED:
            set_version(repo / 'PKGBUILD', args.version)
        shutil.chown(repo, 'builder')
        for path in repo.rglob('*'):
            shutil.chown(path, 'builder')
        if name in VERSIONED:
            run('updpkgsums', cwd=repo, user='builder')
        srcinfo = run('makepkg', '--printsrcinfo', cwd=repo, user='builder', capture=True)
        (repo / '.SRCINFO').write_text(srcinfo)
        print(f'{name}: rendered in {repo}')
        if not args.push:
            continue
        # Rendering runs as builder; restore ownership before root invokes Git.
        shutil.chown(repo, os.getuid(), os.getgid())
        for path in repo.rglob('*'):
            shutil.chown(path, os.getuid(), os.getgid())
        git = ['git', '-C', str(repo), '-c', 'user.name=Colecto release bot', '-c', 'user.email=releases@colecto.invalid']
        subprocess.run([*git, 'add', 'PKGBUILD', '.SRCINFO'], check=True)
        if subprocess.run([*git, 'diff', '--cached', '--quiet']).returncode == 0:
            print(f'{name}: already up to date')
            continue
        subprocess.run([*git, 'commit', '-m', f'Update to {args.version}'], check=True)
        subprocess.run([*git, 'push', 'origin', 'HEAD:master'], check=True, env=env)
        print(f'{name}: pushed')


if __name__ == '__main__':
    main()
