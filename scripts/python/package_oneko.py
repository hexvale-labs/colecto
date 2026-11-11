"""Package the Oneko guest and its sprite, using the caller's Cargo target directory."""
import os
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED
root = Path(__file__).resolve().parents[2]
source = root / 'docs/extensions/examples/oneko'
target = Path(os.environ.get('CARGO_TARGET_DIR', source / 'target'))
module = (target / 'wasm32-unknown-unknown/release/oneko.wasm').read_bytes()
with ZipFile(source.with_suffix('.colectoext'), 'w', ZIP_DEFLATED) as package:
    package.writestr('manifest.toml', (source / 'extension.toml').read_bytes())
    package.writestr('module.wasm', module)
    package.write(source / 'assets/oneko.png', 'assets/oneko.png')
