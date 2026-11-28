#!/usr/bin/env bash
# sakura-twilight-simpler stereo timing: MSAA 2x/4x x bloom off/on (4 runs),
# from the local player's spawn with a gently moving head and no peer (the
# remote avatar is still built and shown, as the scene does today).
# FRAMES is simulated frames at 75 Hz: 1500 = 20 s per run.
# Results: $OUT/results.md and results.csv, plus a JSON report and log per run.
set -euo pipefail

cd "$(dirname "$0")/.."

FRAMES="${FRAMES:-1500}"
WARMUP="${WARMUP:-300}"
SIZE="${SIZE:-1920}"
OUT="${OUT:-/tmp/sakura-simpler-matrix}"
MSAA_MODES="${MSAA_MODES:-2x 4x}"
BLOOM_MODES="${BLOOM_MODES:-off on}"
mkdir -p "$OUT"

cargo test --release --lib profile_ship_stereo --no-run

for msaa in $MSAA_MODES; do
    for bloom in $BLOOM_MODES; do
        name="sakura-msaa${msaa}-bloom${bloom}"
        echo "== $name"
        env SHIP_PROFILE_SOURCE=examples/sakura-twilight-simpler.mms SHIP_PROFILE_ROOM=scene \
            SHIP_PROFILE_CASE=full SHIP_PROFILE_MOTION=head \
            SHIP_PROFILE_MSAA="$msaa" SHIP_PROFILE_BLOOM="$bloom" \
            SHIP_PROFILE_WIDTH="$SIZE" SHIP_PROFILE_HEIGHT="$SIZE" \
            SHIP_PROFILE_GPU_PASSES=1 SHIP_PROFILE_UPDATE_TIMINGS=1 \
            SHIP_PROFILE_WARMUP="$WARMUP" SHIP_PROFILE_FRAMES="$FRAMES" \
            SHIP_PROFILE_OUTPUT="$OUT/$name.json" \
            cargo test --release --lib profile_ship_stereo -- --ignored --nocapture \
            > "$OUT/$name.log" 2>&1 || { echo "   failed, see $OUT/$name.log"; continue; }
    done
done

python3 "$(dirname "$0")/summarize-interceptor-matrix.py" "$OUT"
