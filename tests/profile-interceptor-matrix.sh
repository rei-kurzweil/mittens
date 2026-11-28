#!/usr/bin/env bash
# Interceptor stereo GPU timing: bloom on/off x MSAA off/2x/4x x zero/two NPCs.
# Each run grabs the spawn-side rocket launcher with the right laser, then
# sweeps the view while firing it (SHIP_PROFILE_MOTION=interceptor_play).
# FRAMES is simulated frames at 75 Hz: 4500 = one minute of play per run.
set -euo pipefail

cd "$(dirname "$0")/.."

FRAMES="${FRAMES:-4500}"
WARMUP="${WARMUP:-300}"
SIZE="${SIZE:-1920}"
OUT="${OUT:-/tmp/interceptor-matrix}"
mkdir -p "$OUT"

cargo test --release --lib profile_ship_stereo --no-run

for npcs in 0 2; do
    for msaa in off 2x 4x; do
        for bloom in off on; do
            name="npcs${npcs}-msaa${msaa}-bloom${bloom}"
            echo "== $name"
            env SHIP_PROFILE_SOURCE=examples/interceptor.mms SHIP_PROFILE_ROOM=interceptor \
                SHIP_PROFILE_CASE=full SHIP_PROFILE_MOTION=interceptor_play \
                SHIP_PROFILE_MSAA="$msaa" SHIP_PROFILE_BLOOM="$bloom" \
                SHIP_PROFILE_INTERCEPTOR_NPCS="$npcs" \
                SHIP_PROFILE_WIDTH="$SIZE" SHIP_PROFILE_HEIGHT="$SIZE" \
                SHIP_PROFILE_GPU_PASSES=1 SHIP_PROFILE_WARMUP="$WARMUP" SHIP_PROFILE_FRAMES="$FRAMES" \
                SHIP_PROFILE_OUTPUT="$OUT/$name.json" \
                cargo test --release --lib profile_ship_stereo -- --ignored --nocapture \
                > "$OUT/$name.log" 2>&1 || { echo "   failed, see $OUT/$name.log"; continue; }
            grep -h "WARNING" "$OUT/$name.log" || true
        done
    done
done

python3 - "$OUT" <<'EOF'
import json, pathlib, sys
out = pathlib.Path(sys.argv[1])
print(f"\n{'run':<28}{'gpu mean':>10}{'gpu p95':>10}{'cpu total':>11}{'held':>7}  (ms per stereo frame)")
for path in sorted(out.glob("npcs*.json")):
    if path.name.endswith(".frames.json"):
        continue
    r = json.loads(path.read_text())
    gpu = r["timing"].get("gpu_graphics_ms", {})
    total = r["timing"].get("total_ms", {})
    held = r.get("weapon_held_fraction")
    held = "-" if held is None else f"{held:.0%}"
    print(f"{path.stem:<28}{gpu.get('mean_ms', float('nan')):>10.2f}{gpu.get('p95_ms', float('nan')):>10.2f}"
          f"{total.get('mean_ms', float('nan')):>11.2f}{held:>7}")
EOF
