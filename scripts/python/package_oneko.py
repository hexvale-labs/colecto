"""Package the Oneko guest and its sprite, using the caller's Cargo target directory."""
import json
import subprocess
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED
root = Path(__file__).resolve().parents[2]
source = root / 'docs/extensions/examples/oneko'
# Ask Cargo: CARGO_TARGET_DIR or build.target-dir in a Cargo config may move the build.
metadata = subprocess.run(
    ['cargo', 'metadata', '--format-version', '1', '--no-deps', '--manifest-path', str(source / 'Cargo.toml')],
    check=True, capture_output=True, text=True,
).stdout
target = Path(json.loads(metadata)['target_directory'])
module = (target / 'wasm32-unknown-unknown/release/oneko.wasm').read_bytes()
with ZipFile(source.with_suffix('.colectoext'), 'w', ZIP_DEFLATED) as package:
    package.writestr('manifest.toml', (source / 'extension.toml').read_bytes())
    package.writestr('module.wasm', module)
    package.write(source / 'assets/oneko.png', 'assets/oneko.png')
