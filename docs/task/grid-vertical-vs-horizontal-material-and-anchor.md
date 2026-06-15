# Grid Vertical vs Horizontal Anchor and Material

Date: 2026-06-14

Status: open

Related:

- `docs/task/grid-tool-and-surface-placement-followups.md`
- `docs/spec/grid-material.md`

## Problem statement

Grid placement currently mixes at least two concerns that need to be separated:

1. placement frame resolution
2. grid visual interpretation of that frame

Recent verification in `bisket-vr-demo` suggests the grid is often being placed from a roughly correct hit, but the final visual still behaves like the wrong kind of grid for the resolved surface:

- floor-like placements still end up looking vertical
- repeated grid placements on the ground box can still look offset from the expected plane

Even before the full placement math is fixed, we need a clearer grid model for:

- horizontally anchored grids
- vertically anchored grids

That distinction should drive which shader/material path is used.

## Immediate design direction

Phase 1 should infer grid orientation from the resolved placement frame angle.

Practical rule:

- if the fitted surface normal is close to world up/down, treat the grid as horizontally anchored
- if the fitted surface normal is closer to a wall-like direction, treat the grid as vertically anchored

This does not need to solve every exotic orientation yet.

The goal is to stop treating all grids as if one visual/material interpretation fits both floor and wall placements.

## Proposed runtime model

Expand grid runtime state with an explicit orientation/anchor classification.

Possible shape:

- `GridAnchorMode::Horizontal`
- `GridAnchorMode::Vertical`

The first version can be derived automatically from the placement frame.

Later extensions could allow:

- explicit authored override
- fully arbitrary/oriented grids
- more than two visual modes

## Material/shader split

The component should carry enough state for rendering to select different grid materials.

Phase 1 recommendation:

- horizontal grids use the current floor-like grid material path
- vertical grids use a separate wall-like grid material path

Expected difference:

- horizontal grids should read like a ground plane
- vertical grids should read like a wall plane
- line orientation, fade, and stripe treatment can differ between the two

This should not be hidden inside vague normal math only.
The renderer/material selection should know which kind of grid it is drawing.

## First-pass inference

Given a resolved placement frame normal `n`:

- compare `abs(dot(n, world_up))` against a threshold
- if above threshold: classify as `Horizontal`
- else: classify as `Vertical`

Suggested first threshold:

- start with something conservative like `0.7` to `0.8`

The exact number can be tuned after runtime verification.

## Open questions

1. Should vertical grids still use the same underlying plane mesh, just with a different material?
2. Should horizontal vs vertical affect only rendering, or also snapping behavior?
3. Should wall-like grids keep a different stripe/band emphasis than floor-like grids?
4. Should the inferred mode be recomputed whenever a preview moves across different surfaces, or frozen at drag start?

## Recommended implementation steps

1. Add an explicit grid anchor/orientation flag to grid runtime state.
2. During grid preview/placement, infer `Horizontal` vs `Vertical` from the resolved placement normal.
3. Route rendering through different material handles for the two modes.
4. Keep the mesh/transform path simple at first; do not mix this task with deeper cursor/frame bug fixes.
5. Verify on:
   - ground box
   - back wall

## Acceptance

- floor-like placements produce a horizontal-style grid visual
- wall-like placements produce a vertical-style grid visual
- the chosen material path is visible and deterministic from the resolved angle
- the grid component stores enough information that the renderer does not need to re-guess intent from scratch
