#!/usr/bin/env bash
# Run each scenario sequentially: concurrent benchmarks would distort CPU timing.
# Usage: scripts/benchmark_avatar_udp.sh [output-directory] [extra benchmark args...]
set -euo pipefail
cd -- "$(dirname -- "$0")/.."
output=${1:-/tmp/mittens-avatar-udp}
if [ "$#" -gt 0 ]; then shift; fi
mkdir -p -- "$output"
for scenario in vr2 vr4 mixed4; do
    scripts/benchmark_udp_transport.sh --avatars "$scenario" --seconds 1 --trials 4 --rounds 500 --mode all "$@" > "$output/$scenario.csv"
    python3 scripts/summarize_udp_transport.py "$output/$scenario.csv" > "$output/$scenario.md"
done
