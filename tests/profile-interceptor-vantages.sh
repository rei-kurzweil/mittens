#!/usr/bin/env bash
# Interceptor vantage sweep: the XR camera holds each of 9 deck sections x 26
# directions (SHIP_PROFILE_MOTION=vantage) while NPCs and scene logic run.
# VANTAGE_FRAMES frames per vantage (the first quarter unsampled): 40 gives
# 9360 frames per run. One run per NPC count in NPCS.
# Results: $OUT/vantages.md (colour-coded grid, hot spots) plus JSON and logs.
set -euo pipefail

cd "$(dirname "$0")/.."

NPCS="${NPCS:-0 2}"
MSAA="${MSAA:-4x}"
BLOOM="${BLOOM:-off}"
SIZE="${SIZE:-1920}"
WARMUP="${WARMUP:-300}"
VANTAGE_FRAMES="${VANTAGE_FRAMES:-40}"
OUT="${OUT:-/tmp/interceptor-vantages}"
mkdir -p "$OUT"

cargo test --release --lib profile_ship_stereo --no-run

for npcs in $NPCS; do
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

python3 "$(dirname "$0")/summarize-interceptor-vantages.py" "$OUT"
