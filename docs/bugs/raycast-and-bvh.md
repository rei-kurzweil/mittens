# Raycast and BVH overlay ordering

## Status

Open bug / investigation note.

## Symptom

Editor-facing overlay content can render on top of opaque scene geometry but still lose picking to the underlying opaque object.

Examples include:

- transform gizmos
- editor interaction planes
- other always-on-top editor affordances

This creates a mismatch between what the user sees and what the raycast system reports as the best hit.

## Current behavior

Rendering and picking are currently decided by different rules:

- rendering uses overlay-phase routing (`OverlayComponent`) so visuals draw after the main scene
- ray selection is driven by raycast/BVH hit ordering, which does not necessarily account for overlay intent

As a result, a visually topmost overlay can still be considered "behind" an opaque object for picking purposes.

## Why this is a problem

For editor tools, visual priority and interaction priority generally need to agree.

If a gizmo handle is visibly on top, the user expects it to win the click unless the editor explicitly chooses otherwise.

## Likely root cause

The hit resolver appears to optimize for geometric nearest-hit semantics, while overlay rendering is a later presentation concern.

That is reasonable for scene picking, but not sufficient for editor affordances where the intended interaction order is:

1. active editor overlays / gizmos
2. editor-local interaction surfaces
3. ordinary scene geometry

## Desired behavior

The raycast winner should be chosen by an editor-aware priority policy, not by raw nearest geometry alone.

A likely initial policy:

1. overlay editor controls
2. other explicit editor interaction surfaces
3. normal raycastable scene content
4. tiebreak by distance within the same class

## Investigation targets

- `RayCastSystem` hit collection and winner selection
- `BvhSystem` result ordering vs fallback brute-force ordering
- how gizmo/editor helper nodes are marked today (`OverlayComponent`, `RaycastableComponent`, editor ancestry)
- whether the winner should be resolved centrally or by editor-specific post-filtering

## Likely implementation direction

Do not teach the BVH about overlay rendering directly.

Instead:

- collect candidate hits as usual
- classify each hit by interaction priority
- choose the best hit by `(priority, distance)` rather than distance alone

This keeps rendering concerns and acceleration structure concerns decoupled while still aligning user interaction with editor presentation.

## Related follow-up

Selected-object gizmos may also need size/priority heuristics so thin or distant handles remain practically pickable.
