# Gesture refactor ideas (ray sources vs drag mapping)

This doc is a proposal for refactoring how drags/gestures are represented.

Motivation:
- Today, `GestureSystem` has an enum that reads like “coordinate source” (formerly `RayCastCoords` vs `ScreenSpaceCoords`).
- In practice, the primary behavioral difference is whether the drag **depends on continuous hit tests** (hovering the same geometry) or uses a **stable projection surface** captured at drag start.
- The current naming is mouse/screen-centric, which doesn’t generalize cleanly to XR controllers or autonomous raycasters.

This doc focuses on the *gesture-level* refactor (how we represent and update a drag), not the raycast implementation details.

Two separate docs/threads:
- This one: naming + architecture boundaries (gesture lifecycle vs hit testing vs mapping).
- `docs/screen-space-gizmo-drag.md`: a separate proposal for making “screen space” mean literal pixel-driven mapping for gizmo handles.

## What we have today

### Inputs
- `RayCastSystem` emits `EventSignal::RayIntersected { raycaster, renderable, t, origin, dir }`.
  - The ray can come from:
    - cursor-through-active-camera, or
    - a “parent forward” transform (non-screen / autonomous ray source).

### Gesture recognition
- `GestureSystem` consumes `RayIntersected` and emits:
  - `DragStart { raycaster, renderable, hit_point }`
  - `DragMove { raycaster, renderable, hit_point, delta_world }`
  - `DragEnd { raycaster, renderable, hit_point }`

### Drag mapping modes
- “Target-contact” mode (`DragUpdatePolicy::RequireTargetContact`): only emits `DragMove` when the pointer still intersects the captured target.
- “Start-plane projection” mode (`DragUpdatePolicy::StartPlaneProjection`): captures a drag plane on `DragStart` and continues to emit `DragMove` by intersecting the current pointer ray with that plane.

Qualitatively, this is less about “screen space” vs “world space”, and more about whether the drag is **continuous-hit** vs **projected-on-a-stable-surface**.

Also, the current enum name bundles multiple concerns together:
- Should a drag continue if the pointer stops hitting the original renderable?
- If it does continue, what constraint defines pointer→world motion (a plane, an axis, a ring, etc.)?
- How do we make sign/direction consistent across camera angles?

## Proposed refactor: split responsibilities

### 1) Pointer ray sources
Introduce a clear abstraction for “something that produces a pointer ray and button state”.

Examples:
- Mouse/cursor pointer (camera-unprojected ray)
- XR controller pointer (aim/grip pose ray)
- Autonomous raycaster entity (parent-forward ray)

Instead of “RayCastSystem owns ray creation”, consider emitting a generic signal snapshot per pointer:

- `PointerRay { pointer_id, origin, dir }`
- `PointerButton { pointer_id, pressed/down/released }`

Then `RayCastSystem` becomes “ray → hit” and is decoupled from *where* the ray came from.

### 2) Gesture recognition
`GestureSystem` should primarily:
- decide when a drag starts/updates/ends
- maintain per-pointer drag state

It should not need to know if the ray was derived from a cursor or a controller.

### 3) Split the current enum into two concepts

The current “coordinate source” is doing two jobs. Splitting them makes naming clearer and makes it easier to add new mappings later.

#### A) Continuation policy (hit-lock vs free)

This is the gesture-level rule:

"Once a drag starts on `(raycaster, renderable)`, do we require the pointer ray to keep intersecting that same renderable to keep emitting drag moves?"

Suggested type name:
- `DragContinuationPolicy`
  - `RequireTargetContact` (formerly “hit lock” / today’s `RayCastCoords` behavior)
  - `FreeAfterStart` (conceptually matches today’s `StartPlaneProjection` behavior)

This concept generalizes cleanly to arbitrary/autonomous raycasters.

#### B) Mapping (how we compute deltas)

This is the math policy:

"Given the current pointer state, how do we compute a stable `delta_world` (or other delta) for this drag?"

Examples:
- `StartPlaneProjection` (what we do today in the “screen space” mode)
- ring-plane angle for rotation gizmos
- closest-point ray-to-axis for translation gizmos
- literal screen-pixel mapping for gizmos (see `docs/screen-space-gizmo-drag.md`)

For gizmos, mapping often depends on *which handle is grabbed* (axis vs ring), so it may belong in `GizmoSystem` rather than being a global policy in `GestureSystem`.

## How autonomous raycasters fit in

Autonomous raycasters still work with either continuation policy, but they need:
- a well-defined pointer ray each tick (which they already have), and
- a press/down/release signal to drive gesture state.

What changes is *what constrains a drag*:
- Start-plane projection mapping can be used with any pointer ray source.
- The plane normal does not need to be “screen” — it can be “the drag-start ray direction”, which is valid for XR controllers and parent-forward rays too.

So the label “screen space” is misleading for the current mapping; it’s really “start-plane projection using the pointer ray”.

## Do we still need `RequireTargetContact` / “continuous hit”?

I think we should keep it as an available policy, but probably stop using it for gizmos.

### Why keep it

There are real interaction types where “only drag while still on the thing” is the desired behavior:
- direct manipulation of geometry (surface editing, painting, sculpting)
- tools where leaving the target should immediately stop the operation
- interactions where the target can change (or become occluded) and you *want* that to cancel the gesture

XR example:
- “push / poke / press with a VR hand”: you generally want the interaction to continue only while the hand is still in contact (or still within a small interaction volume) on the same target.
  - If the hand is modeled as a ray/pointer, this corresponds directly to `RequireTargetContact`.
  - If the hand is modeled as a collider/overlap volume, it’s the same policy conceptually (still “hit-locked”), just driven by overlap/contact rather than a ray intersection.

This remains true even if the ray source is not a mouse/camera (e.g. an XR controller ray, or an autonomous entity ray).

### Why not use it for gizmos

For editor gizmos, `RequireTargetContact` tends to feel bad:
- thin handles are easy to “slip off” while dragging
- the moved object/handle can occlude itself
- minor picking/narrow-phase fluctuations can cause intermittent hit loss

So for gizmos, a `FreeAfterStart` continuation policy is usually the right default.

### Practical recommendation

- Keep `RequireTargetContact` for future non-gizmo gestures.
- (Rename-wise, prefer `RequireTargetContact` to make it clear this can be ray hit or overlap/contact.)
- Default gizmos to `FreeAfterStart`.
- Make the mapping itself an independent choice (plane projection today; potentially screen-driven mapping later).

## Minimal incremental steps

1. Rename the enum and fields (no behavior change):
  - `DragCoordinateSource` → `DragUpdatePolicy`
  - `RayCastCoords` → `RequireTargetContact`
  - `ScreenSpaceCoords` → `StartPlaneProjection`

  The important part is: stop calling ray-plane projection “screen space”.

2. Make `DragStart` carry the captured projection plane explicitly:
   - add `drag_plane_point_world` and `drag_plane_normal_world` (or a compact representation)

   This removes the hidden coupling where Gizmo debugging/consumers must look up the start ray indirectly.

3. Add `pointer_id` and make gesture state per pointer.

4. Introduce a `PointerRay`/`PointerButton` signal layer (optional), moving ray creation out of raycast.

## Notes on UX and math

- `FreeAfterStart` is good for “editor gizmo feel”: it keeps dragging stable even when the cursor is no longer hovering the handle geometry.
- `RequireTargetContact` is good for “direct manipulation on geometry”: you only move while still hitting.
- For axis drags, `delta_world` can be computed using plane projection (current) and then projected onto the axis.
- A future alternative mapping for axis drags is “closest point between pointer ray and axis line”, which directly yields the scalar movement along the axis.
