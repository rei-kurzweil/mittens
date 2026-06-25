# Grid Snapping

This document describes the current grid-snapping behavior across:

- transform gizmo translation
- paint placement / `Free Draw`
- paint placement preview
- grid tool placement and preview

It is an overview of the current implementation and the intended direction.
It is not yet a strict final spec.

## Goals

Grid snapping should make placement and manipulation feel coherent across editor workflows.

Today that means:

- when a grid is active, editor operations should be able to quantize motion or placement to that grid
- snapping should respect the grid's transform, not just world XZ
- snapping should preserve the parts of motion that are not supposed to be quantized
- gizmo motion should remain stable under object rotation and parent transforms
- previews and committed placement should agree as closely as possible

## Shared Concepts

### Active grid

Most snapping paths start by resolving an `ActiveGrid`.

An `ActiveGrid` carries:

- spacing
- world transform
- inverse world transform
- surface normal

This lets snapping happen in grid-local space and then map back to world space.

See:

- [src/engine/ecs/system/grid_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/grid_system.rs:24)

### Grid-local quantization

The common snapping model is:

1. transform a world-space point into grid-local space
2. quantize the in-plane coordinates by `spacing`
3. keep or replace the off-plane coordinate depending on the operation
4. transform the snapped point back to world space

There are currently two concrete snapping helpers:

- `GridSystem::snap_hit(...)`
- `GridSystem::snap_point_preserving_plane_offset(...)`

See:

- [src/engine/ecs/system/grid_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/grid_system.rs:250)

## Current Helpers

### `snap_hit`

`GridSystem::snap_hit(...)` is the original placement-oriented helper.

Behavior:

- snap grid-local X/Z to the nearest cell
- force grid-local Y to `0.0`
- return the snapped world-space point and grid normal

This is appropriate when the snap result is supposed to land on the grid surface itself.

That is why it is currently used by paint and placement flows that want a surface frame.

### `snap_point_preserving_plane_offset`

`GridSystem::snap_point_preserving_plane_offset(...)` is the current gizmo-oriented helper.

Behavior:

- snap grid-local X/Z to the nearest cell
- preserve grid-local Y
- return the snapped world-space point and grid normal

This is appropriate when the object should stay at its current offset from the grid plane while its in-plane coordinates quantize.

That is why it is currently used for gizmo translation snapping.

## Gizmo Snapping

### Current behavior

Gizmo translation no longer accumulates per-frame drag deltas directly.

Instead it:

1. captures drag-start hit point in world space
2. captures drag-start target translation
3. computes drag displacement from the current hit point relative to drag start
4. projects that displacement onto the active gizmo axis
5. converts that axis delta into the target translation space
6. applies grid snapping if there is an active grid

See:

- [src/engine/ecs/system/gizmo_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/gizmo_system.rs:349)
- [src/engine/ecs/system/gizmo_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/gizmo_system.rs:742)

### Important detail: translation uses parent space

The target transform's translation channel is interpreted in parent space, not in the target's own rotated space.

That means world/local conversion for snapped translation now goes through the parent transform matrix:

- `target_translation_local_to_world(...)`
- `world_point_to_target_translation_local(...)`

This avoids rotation-coupled drift or Y wobble when dragging a rotated object in world-space gizmo mode.

See:

- [src/engine/ecs/system/gizmo_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/gizmo_system.rs:295)
- [src/engine/ecs/system/gizmo_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/gizmo_system.rs:309)

### What snapping means for gizmos

For gizmo translation, snapping currently means:

- constrain drag to the selected gizmo axis
- quantize the candidate point in grid space
- preserve distance from the grid plane
- convert the snapped world point back into the target's translation space

The concrete snapping helper used here is:

- `GridSystem::snap_point_preserving_plane_offset(...)`

See:

- [src/engine/ecs/system/gizmo_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/gizmo_system.rs:761)

## Free Draw Placement

### Initial placement

Initial `Free Draw` placement goes through the paint placement pipeline.

The paint path resolves `grid_snap` from the hit renderable via:

- `PaintContext::grid_snap(...)`

That currently returns:

- `GridSystem::snap_hit(&grid, hit_point)`

Then placement resolves a `SurfacePlacementFrame` using either:

- the snapped grid point and grid normal, or
- the unsnapped hit point and a resolved surface normal

Finally the asset pose is derived from that frame.

See:

- [src/engine/ecs/system/editor_paint_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_paint_system.rs:1157)
- [src/engine/ecs/system/paint_placement.rs](/home/rei/_/cat-engine/src/engine/ecs/system/paint_placement.rs:44)
- [src/engine/ecs/system/paint_placement.rs](/home/rei/_/cat-engine/src/engine/ecs/system/paint_placement.rs:84)

### What snapping means for initial Free Draw placement

For initial `Free Draw` placement, snapping currently means:

- snap the hit point onto the active grid surface
- use the grid normal as the placement normal
- build the placement pose from that snapped surface frame

This is a placement-frame snap, not an axis-motion snap.

## Free Draw Preview

### Preview creation

When preview begins for `Free Draw`, the preview root is spawned and immediately positioned using the same placement-frame path:

- resolve `grid_snap`
- resolve `SurfacePlacementFrame`
- resolve aligned placement pose
- apply the pose to the preview root

See:

- [src/engine/ecs/system/editor_paint_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_paint_system.rs:1263)

### Preview updates while dragging

After preview exists, drag updates continue to recompute:

- `grid_snap`
- `SurfacePlacementFrame`
- aligned placement pose

and then move the preview root.

See:

- [src/engine/ecs/system/editor_paint_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_paint_system.rs:754)

### Shared behavior with initial placement

The lower-level placement math is mostly shared between:

- initial `Free Draw` placement
- `Free Draw` preview creation
- `Free Draw` preview drag updates

They all depend on:

- `PaintContext::grid_snap(...)`
- `resolve_surface_placement_frame(...)`
- `resolve_surface_aligned_pose_from_frame(...)`

### Why preview and initial placement can still feel different

Even though the frame construction is shared, preview and initial placement are not the same interaction path.

Differences include:

- preview exists only after preview-session startup
- preview updates happen on drag move
- initial placement can occur from click handling without the same temporal history
- gesture timing and target continuity can differ between click placement and drag-driven preview updates

So the current code shares math more than it shares UX.

## Grid Tool

### Grid preview

Grid tool preview also starts from a snapped placement frame:

- resolve `grid_snap` using `snap_hit(...)`
- resolve placement frame
- resolve pose from that frame
- feed that pose into `GridSpawnSpec::from_cursor_pose(...)`

See:

- [src/engine/ecs/system/editor_paint_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_paint_system.rs:1301)

### Grid placement

The grid tool preview path currently determines the grid pose before the actual grid is committed.

The grid spawn path itself uses cursor translation and rotation from the preview/editor context and converts world-space cursor pose into editor-local transform space during spawn.

See:

- [src/engine/ecs/system/grid_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/grid_system.rs:300)

### What snapping means for the grid tool

For the grid tool, snapping currently means:

- snap the grid preview/placement anchor onto the active grid surface frame
- orient the new grid from that frame

This is conceptually closer to paint placement than to gizmo dragging.

## What Is Common Today

The following are common across most current grid-snapped workflows:

- snapping is resolved from an `ActiveGrid`
- snapping is done by converting into grid-local space and back
- the active grid provides both snapped point and surface normal
- placement-style workflows build a surface frame from snapped point + normal

The following are not common yet:

- a single shared definition of what "snapped" means
- a single shared helper for all editor workflows
- a single shared interaction model between click placement, preview movement, and gizmo dragging

## Current Split in Snap Semantics

Today there are really two snapping semantics:

### Surface-frame snapping

Used by:

- free draw placement
- free draw preview
- grid tool preview / placement

Meaning:

- snap to the grid surface itself
- use the grid normal as the placement frame normal

Current helper:

- `snap_hit(...)`

### Motion-preserving snapping

Used by:

- gizmo translation

Meaning:

- quantize in-plane coordinates
- preserve off-plane offset
- preserve manipulation intent rather than force contact with the grid surface

Current helper:

- `snap_point_preserving_plane_offset(...)`

## Direction / Goals

The likely long-term model is:

- placement workflows use surface-frame snapping
- manipulation workflows use motion-preserving snapping

That split is coherent if we make it explicit and document it as policy rather than as an accident of implementation.

We should aim for:

- clear naming around snap mode
- fewer hidden differences between preview and commit
- fewer path-specific assumptions in tool code
- explicit tests for each snapping mode

## Issues / Open Questions

- `Free Draw` preview and initial click placement appear to share lower-level snap math, but they may still feel different because preview is drag-driven and placement can be click-driven. We should verify whether there is a user-visible mismatch in when snapping begins or which target continuity assumptions apply.
- `PaintContext::grid_snap(...)` currently always uses `GridSystem::snap_hit(...)`. That is correct for placement-frame snapping, but it means paint placement and previews do not currently have a motion-preserving snap mode.
- Gizmo snapping now has a dedicated helper, but there is no formal shared enum or policy type naming the two snap semantics.
- It is still unclear whether grid-tool commit and grid-tool preview are perfectly identical in all cases, or whether there are edge cases where preview pose and final spawned pose diverge.
- Selection issues are still unresolved. Since active-grid selection feeds gizmo snapping context, selection bugs can still indirectly affect snapping behavior even if the snapping math is correct.
- We may want a higher-level `SnapMode` concept, for example:
  - `SnapMode::SurfaceContact`
  - `SnapMode::PreservePlaneOffset`
- The current docs for gizmos and placement discuss pieces of this behavior, but there is not yet a single authoritative snap contract shared across editor systems.
