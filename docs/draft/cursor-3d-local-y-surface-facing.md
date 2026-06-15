# Cursor 3D: Local `+Y` Should Face the Surface

Date: 2026-06-15

Status: draft

## Goal

Change the `cursor_3d` surface-placement contract so that the cursor's local `+Y`
axis points in the direction the hit surface is facing.

In practical terms:

- if the cursor is placed on a floor, local `+Y` should point upward
- if the cursor is placed on a wall, local `+Y` should point outward from the wall
- if the cursor is placed on a ceiling, local `+Y` should point downward

This should be true relative to the surface frame, not relative to world up.

## Current behavior

Today `cursor_3d` delegates to the shared surface-placement helpers in
[`paint_placement.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/paint_placement.rs:84).

The current frame builder does this:

- derive `surface_normal_world`
- build a basis where local `+Z` is aligned to that surface normal
- derive the placement quaternion from that basis

That means the current placement contract is:

- local `+Z` = surface-facing axis
- local `+Y` = tangent/up-like axis projected from a reference up vector

For the cursor, that is the wrong axis contract if we want the cursor's own
local up axis to represent the outward-facing direction of the surface.

## Desired behavior

For `cursor_3d` surface placement, the contract should become:

- local `+Y` = `surface_normal_world`
- local `+X` and local `+Z` span the tangent plane
- translation still lands on the hit point with the same small outward offset policy

This is specifically about the cursor's local frame.

It does not automatically mean every other surface-placement consumer should also
switch from `+Z`-aligned placement to `+Y`-aligned placement.

## Why this change is needed

The cursor is acting like a workspace placement frame. If its local axes are used
visually or semantically by later tools, then the axis that reads as "up from the
surface" should be the cursor's local `+Y`.

That gives the more intuitive result:

- the cursor's local up matches the surface normal
- tools that consume the cursor pose can treat `+Y` as the cursor's outward axis
- the cursor's orientation contract becomes easier to reason about for grids,
  stamps, and future surface-authored tools

## Scope

Minimum scope:

1. update `cursor_3d` surface placement so the stored cursor rotation uses a
   frame with local `+Y` aligned to the surface normal
2. keep cursor translation and outward offset behavior unchanged
3. audit cursor visual geometry so it actually communicates the same axis basis
   as the stored rotation

Likely non-goals for this draft:

- changing every placement system in the engine to a `+Y`-normal contract
- changing selection or gizmo behavior
- changing `Select + Cursor` transform-copy mode

## Recommended implementation shape

There are two plausible approaches.

### Option A: Cursor-local remap on top of the existing surface frame

Keep the shared placement helpers as they are today:

- shared placement frame remains `+Z` aligned to surface normal

Then add a cursor-specific basis remap:

- convert the shared surface-aligned rotation into a cursor rotation where
  cursor-local `+Y` becomes the outward axis

This is the lower-risk option if other systems already depend on the existing
`+Z`-normal contract.

Conceptually:

- shared placement frame: `+Z = normal`
- cursor local authoring convention: `+Y = normal`
- cursor system composes a fixed corrective rotation between those conventions

### Option B: Add an explicit axis contract to placement-frame construction

Generalize the frame builder so callers can request which local axis should align
to the surface normal.

Possible API direction:

- `NormalAlignedAxis::PositiveY`
- `NormalAlignedAxis::PositiveZ`

Then:

- `cursor_3d` requests `PositiveY`
- existing paint/grid consumers can keep requesting `PositiveZ` until migrated

This is cleaner architecturally, but broader in scope.

## Basis definition

If the cursor uses local `+Y = normal`, then the frame should be built as:

- `y = normalize(surface_normal_world)`
- choose a stable tangent reference not parallel to `y`
- `x = normalize(cross(reference, y))`
- `z = normalize(cross(x, y))`

Or the equivalent handedness-preserving variant already used by the engine.

The important part is the contract:

- the resulting quaternion must transform cursor-local `[0, 1, 0]` into the
  world-space surface normal

That should become the direct invariant for tests.

## Risks

### 1. Cursor visual may be authored in a different basis

Even if the stored rotation becomes correct, the visual cursor mesh/planes may
still appear wrong if their authored geometry assumes a different local axis as
"up" or "face normal".

So this change needs a visual audit, not just math changes.

### 2. Grid or other tools may implicitly assume cursor `+Z` is outward

If downstream code consumes cursor rotation and interprets local `+Z` as the
placement-facing direction, those tools may become inconsistent after the change.

That is why the contract should be documented explicitly rather than changed
silently.

## Suggested tests

### Unit tests

- floor hit: cursor-local `+Y` maps to world `[0, 1, 0]`
- wall hit: cursor-local `+Y` maps to the wall outward normal
- ceiling hit: cursor-local `+Y` maps to world `[0, -1, 0]`

### Integration checks

- `3D Cursor` on horizontal surfaces visually shows `+Y` pointing away from the surface
- `3D Cursor` on vertical surfaces visually shows `+Y` pointing away from the surface
- any tool that spawns from cursor pose is checked for axis-contract regressions

## Recommended next step

Implement this as a cursor-specific contract first, not a global placement
rewrite:

1. keep shared surface placement `+Z`-aligned unless there is a stronger reason
   to change all consumers
2. remap `cursor_3d` rotation so cursor-local `+Y` is the surface-facing axis
3. document that invariant in code and tests
