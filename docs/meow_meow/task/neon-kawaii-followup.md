# Task: Neon Kawaii Example Phase 2

Date: 2026-07-08

## Phase 1 landed

- `examples/neon-kawaii.mms` exists and uses a reusable background module:
  - `assets/components/backgrounds/star_kawaii_background.mms`
- The background module exports only the inner transform tree, so callers wrap it in `BG { ... }`.
- Stars are placed procedurally on a fixed-radius sphere using `Math.sin` / `Math.cos` and a deterministic sine-hash.

## Phase 2 target

Replace the current inward-facing Euler approximation with a real "point object at target" API in MMS.

## Acceptance criteria

- Authors can orient a transform toward another point without hand-deriving Euler angles.
- Works for generic meshes, not just flat star billboards.
- Clear semantics around up-vector and roll.

## Candidate APIs

- `t.look_at([0, 0, 0])`
- `T.look_at(x, y, z) { ... }`
- `Math.look_at_quat(from, to, up?)`
- `Math.look_at_euler(from, to, up?)`
