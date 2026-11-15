#!/usr/bin/env bash
# Build a small optimized harness with the production wire decoders, not Vulkan.
# Dependencies must already be cached. Arguments are forwarded to the benchmark.
set -euo pipefail
cd -- "$(dirname -- "$0")/.."
python3 - "$@" <<'PY'
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib

root = Path.cwd()
lock = tomllib.loads((root / 'Cargo.lock').read_text())
versions = {entry['name']: entry['version'] for entry in lock['package']}
target = root / 'target/udp-transport-bench'
with tempfile.TemporaryDirectory(prefix='mittens-udp-transport-') as directory:
    package = Path(directory)
    source = package / 'src'
    source.mkdir()
    manifest = '[package]\nname="mittens-udp-transport-bench"\nversion="0.0.0"\nedition="2024"\n[dependencies]\n'
    manifest += f'serde = {{ version="={versions["serde"]}", features=["derive"] }}\n'
    manifest += f'serde_json = "={versions["serde_json"]}"\n'
    manifest += f'uuid = {{ version="={versions["uuid"]}", features=["v4","serde"] }}\n'
    manifest += f'slotmap = "={versions["slotmap"]}"\n'
    manifest += 'meow-meow-script = { path=' + json.dumps(str(root / 'crates/meow-meow-script')) + ' }\n'
    manifest += f'libc = "={versions["libc"]}"\n'
    (package / 'Cargo.toml').write_text(manifest)
    (package / 'Cargo.lock').write_text((root / 'Cargo.lock').read_text())
    for path in (root / 'benchmarks/udp_transport').glob('*.rs'):
        shutil.copyfile(path, source / path.name)
    shutil.copyfile(root / 'src/engine/udp/mod.rs', source / 'udp.rs')
    (source / 'udp').mkdir()
    shutil.copyfile(root / 'src/engine/udp/tests.rs', source / 'udp/tests.rs')
    shutil.copyfile(root / 'src/utils/math.rs', source / 'math.rs')
    shutil.copytree(root / 'src/engine/udp_channels', source / 'udp_channels')
    with (source / 'udp_channels/mod.rs').open('a') as wrapper:
        wrapper.write("\n" + (root / 'benchmarks/udp_transport/channel_adapter.inc').read_text())
    # Extract the self-contained production OSC/HTC decoders without changing its
    # visibility or pulling the engine/ECS/render dependencies into the harness.
    eye = (root / 'src/engine/ecs/system/xr_eye_tracking_system.rs').read_text()
    start = eye.index('pub fn decode_osc(')
    end = eye.index('#[derive(Debug, Clone, PartialEq)]\npub struct HtcEye', start)
    htc_end = eye.index('#[cfg(test)]', end)
    (source / 'eye.rs').write_text('pub type Look = [f32; 3];\nuse crate::math::quat_rotate_vec3;\n' + eye[start:htc_end])
    shutil.copyfile(root / 'src/engine/ecs/system/multiplayer_system/protocol.rs', source / 'protocol.rs')
    subprocess.run(['cargo','build','--offline','--release','--manifest-path',str(package / 'Cargo.toml'),
                    '--target-dir',str(target)],check=True)
    if os.environ.get('BENCH_TEST') == '1':
        subprocess.run(['cargo','test','--offline','--release','--manifest-path',str(package / 'Cargo.toml'),
                        '--target-dir',str(target),'avatars::tests'],check=True, stdout=sys.stderr)
    command = [str(target / 'release/mittens-udp-transport-bench'), *sys.argv[1:]]
    if os.environ.get('PERF') == '1':
        command = ['perf','stat','-e','cycles,instructions,cache-references,cache-misses', '--', *command]
    subprocess.run(command,check=True)
PY
