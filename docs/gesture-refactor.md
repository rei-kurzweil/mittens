# Gesture refactor ideas (ray sources vs drag mapping)

This doc is a proposal for refactoring how drags/gestures are represented.

Motivation:
- Today, `GestureSystem` has an enum that reads like “coordinate source” (`RayCastCoords` vs `ScreenSpaceCoords`).
- In practice, the primary behavioral difference is whether the drag **depends on continuous hit tests** (hovering the same geometry) or uses a **stable projection surface** captured at drag start.
- The current naming is mouse/screen-centric, which doesn’t generalize cleanly to XR controllers or autonomous raycasters.

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
- “Hit-locked” mode (`RayCastCoords`): only emits `DragMove` when the ray continues to hit the same renderable.
- “Plane-projected” mode (`ScreenSpaceCoords`): captures a drag plane on `DragStart` and continues to emit `DragMove` by intersecting the current pointer ray with that plane.

Qualitatively, this is less about “screen space” vs “world space”, and more about whether the drag is **continuous-hit** vs **projected-on-a-stable-surface**.

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

### 3) Drag mapping (the thing we’re currently calling "coordinate source")
Rename the current enum to reflect the actual behavior. Suggested options:

- `DragTrackingMode`:
  - `ContinuousHit` (current `RayCastCoords`)
  - `ProjectedPlane` (current `ScreenSpaceCoords`)

or

- `DragUpdatePolicy`:
  - `RequireHitLock`
  - `ProjectOntoStartPlane`

This makes the meaning device-agnostic.

## How autonomous raycasters fit in

If we remove the continuous-hit mode entirely, autonomous raycasters still work, but they would need:
- a well-defined pointer ray each tick (which they already have), and
- a press/down/release signal to drive gesture state.

What changes is *what constrains a drag*:
- `ProjectedPlane` can be used with any pointer ray source.
- The plane normal does not need to be “screen” — it can be “the drag-start ray direction”, which is valid for XR controllers and parent-forward rays too.

So the label “screen space” is misleading; it’s really “pointer-plane projection”.

## Minimal incremental steps

1. Rename the enum and fields (no behavior change):
   - `DragCoordinateSource` → `DragTrackingMode`
   - `RayCastCoords` → `ContinuousHit`
   - `ScreenSpaceCoords` → `ProjectedPlane`

2. Make `DragStart` carry the captured projection plane explicitly:
   - add `drag_plane_point_world` and `drag_plane_normal_world` (or a compact representation)

   This removes the hidden coupling where Gizmo debugging/consumers must look up the start ray indirectly.

3. Add `pointer_id` and make gesture state per pointer.

4. Introduce a `PointerRay`/`PointerButton` signal layer (optional), moving ray creation out of raycast.

## Notes on UX and math

- `ProjectedPlane` is good for “editor gizmo feel”: it keeps dragging stable even when the cursor is no longer hovering the handle geometry.
- `ContinuousHit` is good for “direct manipulation on geometry”: you only move while still hitting.
- For axis drags, `delta_world` can be computed using plane projection (current) and then projected onto the axis.
- A future alternative mapping for axis drags is “closest point between pointer ray and axis line”, which directly yields the scalar movement along the axis.
