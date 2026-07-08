# Task: Math Built-ins and Kawaii Background Follow-ups

Date: 2026-07-08

## What exists now

- MMS runtime built-ins were limited to `print`, `assert`, `range`, query helpers, and `MusicNote`.
- This change adds a minimal `Math` built-in table with:
  - constants: `Math.pi`, `Math.tau`, `Math.e`
  - functions: `Math.sin`, `Math.cos`, `Math.tan`, `Math.atan`, `Math.atan2`, `Math.floor`, `Math.ceil`, `Math.round`, `Math.abs`
- `examples/neon-kawaii.mms` and `assets/components/backgrounds/star_kawaii_background.mms` now use `Math.sin`/`Math.cos` plus a deterministic sine-hash instead of a true random builtin.

## Gaps still open

- No true `Math.random()` or seeded RNG API.
- No `Math.perlin()` or other noise functions.
- No first-class look-at / point-at / face-target transform helper in MMS.
- `star_kawaii_background.mms` currently approximates "face the center" with Euler `rotation(-pitch, yaw + pi, twist)`. That is good enough for flat stars facing inward on a sphere, but it is not a general look-at solution.

## Recommended next phase

1. Decide whether `Math` is a permanent evaluator builtin table or a temporary bridge until stdlib modules land.
2. Add deterministic RNG helpers before adding a stateful global `random()`.
3. Add noise as either:
   - `Math.perlin(x, y)` / `Math.perlin3(x, y, z)` host builtins, or
   - `std:noise` once the stdlib/module path is ready.
4. Design a transform-facing helper for orientation:
   - `T.look_at([x, y, z])`
   - `Math.look_at_euler(from, to)` returning `[pitch, yaw, roll]`
   - or `Math.look_at_quat(from, to)` returning `[x, y, z, w]`

## Open design questions

- Should `Math.random()` be stateful and nondeterministic, or should MMS prefer explicit seeded functions for reproducibility?
- Does `Math.perlin` belong on `Math`, or in a separate `Noise` / `std:noise` namespace?
- Should future transform mutation expose `update_transform_quat(...)` directly in MMS method dispatch, or keep Euler authoring as the default surface?
