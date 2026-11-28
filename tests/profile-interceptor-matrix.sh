#!/usr/bin/env bash
# Interceptor stereo GPU timing: bloom on/off x MSAA off/2x/4x x 0/1/2 NPCs,
# 18 runs. Each run grabs the spawn-side rocket launcher with the right laser,
# then walks about, looks around and fires every 2 s while the NPCs run their
# full logic (SHIP_PROFILE_MOTION=interceptor_play).
# FRAMES is simulated frames at 75 Hz: 4500 = one minute of play per run.
# Results: $OUT/results.md and $OUT/results.csv, plus a JSON report and log
# per run. NPCS="0 2", MSAA_MODES="4x" or BLOOM_MODES="off" narrow the matrix.
set -euo pipefail

cd "$(dirname "$0")/.."

FRAMES="${FRAMES:-4500}"
WARMUP="${WARMUP:-300}"
SIZE="${SIZE:-1920}"
OUT="${OUT:-/tmp/interceptor-matrix}"
NPCS="${NPCS:-0 1 2}"
MSAA_MODES="${MSAA_MODES:-off 2x 4x}"
BLOOM_MODES="${BLOOM_MODES:-off on}"
mkdir -p "$OUT"

cargo test --release --lib profile_ship_stereo --no-run

for npcs in $NPCS; do
    for msaa in $MSAA_MODES; do
        for bloom in $BLOOM_MODES; do
            name="npcs${npcs}-msaa${msaa}-bloom${bloom}"
            echo "== $name"
            env SHIP_PROFILE_SOURCE=examples/interceptor.mms SHIP_PROFILE_ROOM=interceptor \
                SHIP_PROFILE_CASE=full SHIP_PROFILE_MOTION=interceptor_play \
                SHIP_PROFILE_MSAA="$msaa" SHIP_PROFILE_BLOOM="$bloom" \
                SHIP_PROFILE_INTERCEPTOR_NPCS="$npcs" \
                SHIP_PROFILE_WIDTH="$SIZE" SHIP_PROFILE_HEIGHT="$SIZE" \
                SHIP_PROFILE_GPU_PASSES=1 SHIP_PROFILE_UPDATE_TIMINGS=1 SHIP_PROFILE_WARMUP="$WARMUP" SHIP_PROFILE_FRAMES="$FRAMES" \
                SHIP_PROFILE_OUTPUT="$OUT/$name.json" \
                cargo test --release --lib profile_ship_stereo -- --ignored --nocapture \
                > "$OUT/$name.log" 2>&1 || { echo "   failed, see $OUT/$name.log"; continue; }
            grep -h "WARNING\|INTERCEPTOR_GRAB" "$OUT/$name.log" || true
        done
    done
done

python3 "$(dirname "$0")/summarize-interceptor-matrix.py" "$OUT"
