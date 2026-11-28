#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

echo "Profiling interceptor with no NPCs..."
env SHIP_PROFILE_SOURCE=examples/interceptor.mms SHIP_PROFILE_ROOM=interceptor SHIP_PROFILE_CASE=full SHIP_PROFILE_GPU_PASSES=1 SHIP_PROFILE_UPDATE_TIMINGS=1 SHIP_PROFILE_WARMUP=240 SHIP_PROFILE_FRAMES=480 SHIP_PROFILE_OUTPUT=/tmp/interceptor-gpu-0.json cargo test --release --lib profile_ship_stereo -- --ignored --nocapture

echo "Profiling interceptor with two NPCs and visibility control off..."
env SHIP_PROFILE_SOURCE=examples/interceptor.mms SHIP_PROFILE_ROOM=interceptor SHIP_PROFILE_CASE=full SHIP_PROFILE_GPU_PASSES=1 SHIP_PROFILE_UPDATE_TIMINGS=1 SHIP_PROFILE_INTERCEPTOR_NPCS=2 SHIP_PROFILE_INTERCEPTOR_NO_MOTION_LOD=1 SHIP_PROFILE_WARMUP=240 SHIP_PROFILE_FRAMES=480 SHIP_PROFILE_OUTPUT=/tmp/interceptor-gpu-2-full.json cargo test --release --lib profile_ship_stereo -- --ignored --nocapture

echo "Profiling interceptor with two NPCs and visibility control on..."
env SHIP_PROFILE_SOURCE=examples/interceptor.mms SHIP_PROFILE_ROOM=interceptor SHIP_PROFILE_CASE=full SHIP_PROFILE_GPU_PASSES=1 SHIP_PROFILE_UPDATE_TIMINGS=1 SHIP_PROFILE_INTERCEPTOR_NPCS=2 SHIP_PROFILE_WARMUP=240 SHIP_PROFILE_FRAMES=480 SHIP_PROFILE_OUTPUT=/tmp/interceptor-gpu-2-lod.json cargo test --release --lib profile_ship_stereo -- --ignored --nocapture

echo "Reports: /tmp/interceptor-gpu-0.json, /tmp/interceptor-gpu-2-full.json and /tmp/interceptor-gpu-2-lod.json"
