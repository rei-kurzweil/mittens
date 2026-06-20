# Gizmo clicks lose to scene geometry behind the selected object

## Status

Open bug / investigation.

## Symptom

Once an editor object is selected and its transform gizmo is visible, clicking the gizmo can fail
because the click resolves to some other raycastable renderable in the editor scene behind it.

In practice this usually means:

- selection works
- the gizmo appears
- trying to grab an axis / handle instead re-hits the selected object or some other editor-owned
  renderable behind the gizmo

So the editor can enter a bad loop where selection succeeds but manipulation cannot start
reliably.

## Repro shape

Common repro:

1. Run an editor scene with normal selectable geometry.
2. Select an object so the transform gizmo appears.
3. Position the camera so ordinary scene geometry is visible behind the gizmo handles.
4. Attempt to click a gizmo handle.
5. Observe that the click often resolves to the object or scene renderable behind the gizmo
   instead of the gizmo handle.

This is especially common when:

- the selected object itself still occupies the same screen region as the gizmo
- gizmo handles are visually thin
- the behind-object renderable is a large simple shape with an easy AABB hit

## Expected behavior

When a visible gizmo handle is under the pointer, it should win interaction against ordinary scene
geometry behind it.

For editor interaction, visual/editor affordance priority should beat raw nearest-scene-object
selection in these cases.

## Actual behavior

Current hit selection appears to prefer the nearest eligible BVH/raycast hit without a separate
interaction-priority class for editor overlays / gizmos.

That means a normal scene renderable can win even when the gizmo is the intended editor
interaction target.

## Likely root cause

The current BVH hit selection path is distance-first:

- [src/engine/ecs/system/bvh_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/bvh_system.rs:276)
  chooses the smallest `t`
- candidate ordering in [src/engine/ecs/system/bvh_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/bvh_system.rs:324)
  is also sorted by ascending `t`
- gesture hit collection in [src/engine/ecs/system/gesture_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/gesture_system.rs:87)
  preserves distance ordering

That is fine for ordinary scene picking, but it is missing an explicit editor interaction priority
layer.

We already have a matching class of bug for overlay/UI hit resolution:

- [docs/bugs/raycast-and-bvh.md](/home/rei/_/cat-engine/docs/bugs/raycast-and-bvh.md:1)
- [docs/bugs/panel-clicks-blocked-by-selectable-scene-objects-behind-ui.md](/home/rei/_/cat-engine/docs/bugs/panel-clicks-blocked-by-selectable-scene-objects-behind-ui.md:1)

This gizmo issue looks like the same architectural problem expressed in a 3D editor-manipulation
path rather than a panel/UI path.

## Why this needs a priority model

We need hit classification, not just more selection filtering.

Without priority classes, any of these can beat a gizmo purely by `t`:

- the selected object itself
- another editor-owned object behind it
- a large ground/wall primitive behind it
- helper renderables that are raycastable for unrelated reasons

The fix should not be "special-case gizmo clicks later" if the ray winner is already wrong.
The winner selection needs to understand interaction intent earlier.

## Proposed direction

Add an interaction-priority concept for raycast winners.

Suggested first-pass policy:

- `overlay` / active gizmo handles: priority `1`
- normal scene/editor geometry: priority `0`
- resolve winner by `(priority desc, distance asc)`

Equivalent phrasing:

1. pick highest interaction priority class
2. within that class, pick nearest hit

This can stay independent from visual draw ordering while still aligning with editor affordances.

## Possible representation shapes

Any of these could work:

1. Extend `RaycastableComponent` with an interaction-priority field.
2. Derive priority from topology, such as nearest `OverlayComponent` ancestor or gizmo ancestry.
3. Add a separate explicit component for raycast priority / interaction class.

For a first pass, explicit data on the raycastable entry is probably the clearest.

Example rough policy:

```text
RaycastPriority
  Normal = 0
  Overlay = 1
```

Future expansion could distinguish:

- active gizmo handles
- other editor helper surfaces
- panel/UI surfaces
- ordinary scene geometry

## Investigation targets

- `src/engine/ecs/system/bvh_system.rs`
  - candidate collection and final winner ordering
- `src/engine/ecs/system/gesture_system.rs`
  - whether it should preserve sorted candidates but defer final winner choice to a
    priority-aware resolver
- `src/engine/ecs/system/editor_system.rs`
  - confirm scene selection is consuming the wrong winner rather than the gizmo never being
    raycastable
- gizmo spawn/materialization path
  - confirm gizmo renderables are actually tagged in a way that can express elevated interaction
    priority

## Open questions

- Should priority be attached to the resolved raycastable owner, the renderable, or both?
- Should overlay priority be generic for all editor affordances, or should gizmos get their own
  stronger class?
- If multiple gizmo subparts overlap, do we still want pure nearest-within-priority, or do some
  handle types need their own ordering later?
- Should non-active gizmos / helper markers share the same priority as the active gizmo?

## Recommended first implementation step

Do not start by changing editor selection logic.

Start by:

1. introducing an interaction-priority field/classification for raycastable hits
2. making gizmo handles resolve to a higher class than ordinary scene geometry
3. choosing ray winners by priority first, then distance

That should address this bug at the layer where the wrong target is currently chosen.
