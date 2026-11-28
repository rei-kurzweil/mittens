#!/usr/bin/env bash
# Interceptor vantage sweep: the XR camera holds each of 9 deck sections x 26
# directions (SHIP_PROFILE_MOTION=vantage) while NPCs and scene logic run.
# VANTAGE_FRAMES simulated 75 Hz frames per vantage, the first quarter
# unsampled: 150 holds each view 2 s and samples 1.5 s (35,100 frames per run).
# One run per NPC count x MSAA mode x bloom mode.
# Defaults: 0 and 2 NPCs x MSAA 2x and 4x, bloom off (4 runs). MSAA_MODES and
# BLOOM_MODES take lists, e.g. BLOOM_MODES="off on".
# Results: $OUT/vantages.md (colour-coded grid, hot spots) plus JSON and logs.
set -euo pipefail

cd "$(dirname "$0")/.."

NPCS="${NPCS:-0 2}"
MSAA_MODES="${MSAA_MODES:-2x 4x}"
BLOOM_MODES="${BLOOM_MODES:-off}"
SIZE="${SIZE:-1920}"
WARMUP="${WARMUP:-300}"
VANTAGE_FRAMES="${VANTAGE_FRAMES:-150}"
OUT="${OUT:-/tmp/interceptor-vantages}"
mkdir -p "$OUT"

cargo test --release --lib profile_ship_stereo --no-run

for npcs in $NPCS; do
  for MSAA in $MSAA_MODES; do
    for BLOOM in $BLOOM_MODES; do
    name="vantage-npcs${npcs}-msaa${MSAA}-bloom${BLOOM}"
    echo "== $name"
    env SHIP_PROFILE_SOURCE=examples/interceptor.mms SHIP_PROFILE_ROOM=interceptor \
        SHIP_PROFILE_CASE=full SHIP_PROFILE_MOTION=vantage SHIP_PROFILE_VANTAGE_FRAMES="$VANTAGE_FRAMES" \
        SHIP_PROFILE_MSAA="$MSAA" SHIP_PROFILE_BLOOM="$BLOOM" SHIP_PROFILE_INTERCEPTOR_NPCS="$npcs" \
        SHIP_PROFILE_WIDTH="$SIZE" SHIP_PROFILE_HEIGHT="$SIZE" \
        SHIP_PROFILE_GPU_PASSES=1 SHIP_PROFILE_WARMUP="$WARMUP" \
        SHIP_PROFILE_OUTPUT="$OUT/$name.json" \
        cargo test --release --lib profile_ship_stereo -- --ignored --nocapture \
        > "$OUT/$name.log" 2>&1 || { echo "   failed, see $OUT/$name.log"; continue; }
    done
  done
done

python3 "$(dirname "$0")/summarize-interceptor-vantages.py" "$OUT"
