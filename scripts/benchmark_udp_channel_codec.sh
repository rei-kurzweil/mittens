#!/usr/bin/env bash
# Optimize the channel codec AND its dependencies without rebuilding the renderer.
# Dependencies must already be cached (for example after cargo test --lib udp_channels).
set -euo pipefail
cd -- "$(dirname -- "$0")/.."
python3 - <<'PY'
import json
from pathlib import Path
import subprocess
import tempfile
import tomllib

root = Path.cwd()
metadata = json.loads(subprocess.check_output(
    ['cargo', 'metadata', '--offline', '--no-deps', '--format-version', '1'], text=True))
lock = tomllib.loads((root / 'Cargo.lock').read_text())
versions = {entry['name']: entry['version'] for entry in lock['package']}
with tempfile.TemporaryDirectory(prefix='mittens-channel-codec-') as directory:
    package = Path(directory)
    (package / 'src').mkdir()
    manifest = '[package]\nname = "mittens-channel-codec-bench"\nversion = "0.0.0"\nedition = "2024"\n[dependencies]\n'
    manifest += 'meow-meow-script = { path = ' + json.dumps(str(root / 'crates/meow-meow-script')) + ' }\n'
    for name in ['serde', 'serde_json', 'slotmap']:
        manifest += f'{name} = "={versions[name]}"\n'
    manifest += f'uuid = {{ version = "={versions["uuid"]}", features = ["v4"] }}\n'
    (package / 'Cargo.toml').write_text(manifest)
    # Retain workspace-locked transitive versions instead of resolving newer cache entries.
    (package / 'Cargo.lock').write_text((root / 'Cargo.lock').read_text())
    (package / 'src/lib.rs').write_text('#[path = ' + json.dumps(str(root / 'src/engine/udp_channels/mod.rs')) + ']\npub mod udp_channels;\n')
    subprocess.run(['cargo', 'test', '--offline', '--release', '--manifest-path', str(package / 'Cargo.toml'),
        '--target-dir', str(Path(metadata['target_directory']) / 'channel-codec-bench'),
        '--', '--ignored', '--exact', 'udp_channels::tests::codec_measurement', '--nocapture'], check=True)
PY
