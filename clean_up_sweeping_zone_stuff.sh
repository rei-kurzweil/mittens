#!/usr/bin/env bash
# Reduce the swept-zone experiment to the listener gate and velocity reference.
# Run from the repository root on the PR branch after committing/stashing other work:
#   bash scripts/strip_swept_zone_experiment.sh [base-ref]
#
# The train scene/assets under examples/ and assets/ are intentionally untouched.
set -euo pipefail

BASE_REF="${1:-origin/main}"

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "Run this script from inside the mittens git repository." >&2
  exit 1
fi

if [[ -n "$(git status --porcelain)" ]]; then
  echo "Working tree is not clean. Commit or stash changes before running this cleanup." >&2
  exit 1
fi

if ! git rev-parse --verify "${BASE_REF}^{commit}" >/dev/null 2>&1; then
  echo "Cannot resolve base ref '${BASE_REF}'. Fetch it or pass a valid base ref." >&2
  exit 1
fi

BASE_COMMIT="$(git merge-base HEAD "$BASE_REF")"
if [[ -z "$BASE_COMMIT" ]]; then
  echo "Could not find a merge base between HEAD and ${BASE_REF}." >&2
  exit 1
fi

# Restore files whose changes belong only to the sweep experiment/benchmark.
# Keep collidable.rs (velocity reference), RxWorld (ListenerGate), and the ECS
# re-exports of ListenerGate intact.
git restore --source="$BASE_COMMIT" --worktree -- \
  Cargo.toml \
  src/engine/ecs/system/collidable_bvh.rs \
  src/engine/ecs/signals/signal.rs \
  docs/task/sweeping-zone-query-events-train-example.md

# Remove experiment-only files. Keep the listener-gating design note as the
# focused documentation for the retained gate.
rm -f \
  benches/listener_gate.rs \
  docs/task/velocity-bound-swept-zones-review-and-recovery.md

# The gate tests should use an existing event kind; ZoneSwept is intentionally
# deferred with the unimplemented sweep producer.
python3 - <<'PY'
from pathlib import Path

path = Path("tests/listener_gate.rs")
text = path.read_text()
old = "SignalKind::ZoneSwept"
if old not in text:
    raise SystemExit(f"Expected {old} in {path}; refusing to edit an unexpected file")
path.write_text(text.replace(old, "SignalKind::ProjectileHit"))
PY

git diff --check
printf '\nCleanup applied. Review the diff before committing:\n'
git status --short
