# Gizmos and Gestures (v3 notes after testing)

This doc is a follow-up to:
- `docs/gizmo-and-gestures.md` (historical)
- `docs/refactors/screen-space-gizmo-drag.md`
- `docs/refactors/gesture-refactor.md`
- `docs/refactors/raycast-driven-by-actions.md`

Goal: capture some post-testing observations and refine the architectural split between:
- pointer-driven gesture lifecycle (press/hold/release, lock/continuation semantics)
- gizmo/handle-specific drag mapping (how pointer motion becomes a 1D/2D/3D delta)

No code changes are proposed here; this is a design note.

## Quick status check

### We have *not* implemented the true screen-space 1D slider yet

Right now, the “screen space” drag behavior in the gesture system is effectively:
- **StartPlaneProjection**: capture a ray-defined plane on drag start, then project subsequent pointer rays onto that plane and take world-space deltas.

That is not the same as the “pixel-space 1D slider” behavior described in `docs/refactors/screen-space-gizmo-drag.md`.

### What the plane projection is good at

Plane projection feels great for translation because:
- it produces stable world-space deltas
- it tolerates small pointer-ray changes without sudden jumps
- it naturally supports “drag continues even if the ray misses the target” (depending on continuation policy)

### Where it breaks down (rotation)

For rotation, the plane approach often feels wrong because:
- a rotation ring is fundamentally a **1D control** (an angle), but plane projection is **2D world motion**
- small changes in camera view / ray direction can create unintuitive angular changes
- the desired UX is often “mouse moves left/right -> rotate”, not “project onto some plane and infer angle from a 2D point”

This is exactly the space where a true screen-space 1D slider mapping tends to work best.

## Key refinement: split “mapping” from “lock/continuation”

During testing, one thing became clear:

- The **drag coordinate mapping mode** should depend on the specific object/handle being dragged.
  - Example: translate gizmo axis handle vs rotate ring handle should not share the same mapping.

- The **drag lock policy** should depend on the pointer.
  - Example: the camera ray pointer might prefer “keep dragging even when off-target” (for desktop/editor feel), while a VR poke pointer might prefer “require contact”.

These are orthogonal decisions and shouldn’t be encoded into a single enum.

## Terminology proposal

### Pointer policy (per pointer)

This answers: “when is a drag allowed to continue?”

Examples:
- **RequireTargetContact**: drag continues only while the pointer continues to hit the captured target (or continues a contact test).
- **AllowOffTargetContinuation**: drag continues based on the pointer button state, even if the pointer ray misses the target.

This is a per-pointer decision because it encodes the semantics of the device / interaction style.

Where it likely lives:
- on `PointerComponent` (which opt-ins a raycaster into being a pointer)
- or on a dedicated pointer config component attached near the pointer

### Handle mapping (per dragged handle)

This answers: “how do we convert pointer motion into a delta?”

Examples:
- **Translate / plane projection (2D -> 3D)**
  - capture a drag plane on start
  - compute world-space delta by projecting pointer rays onto that plane
- **Rotate / screen-space 1D slider (pixel delta -> angle)**
  - compute pixel delta (or a 1D projected delta along a screen axis)
  - convert to signed angle using camera/view + ring axis for sign
- **Scale / 1D slider**
  - typically also best as a 1D slider (mouse up/down = scale)

This is a per-handle decision because translation/rotation/scale are different widgets with different UX expectations.

Where it likely lives:
- on the gizmo handle components themselves
- or in a “gizmo interaction config” component referenced by gizmos/handles

## Spec: `GestureCoordTypeComponent` (per handle / per mapping)

This is the “handle mapping” configuration made explicit as a component.

Intent: any raycastable renderable under a gizmo handle subtree should “inherit” how drag motion is interpreted.

### Placement (how we attach it)

We already have a clear “handle root” component per operation:
- translate handles are rooted at nodes with `GizmoTranslateComponent`
- rotate handles are rooted at nodes with `GizmoRotateComponent`
- scale handles will be rooted at nodes with `GizmoScaleComponent`

So the simplest spec is:
- attach a `GestureCoordTypeComponent` at (or above) each TRS handle root node
- all raycastable children under that handle automatically use the correct mapping

This matches your intent (“the thing that has the T/R/S subcomponents configures the mapping”), without having to special-case individual renderables.

### Proposed shape

Name: `GestureCoordTypeComponent` (or `GestureCoordMappingComponent`).

Core data it needs:
- mapping kind (plane/world delta vs screen 1D slider)
- a few mapping parameters (usually sensitivity)

Sketch:
- `PlaneWorldDelta`
  - meaning: consume `delta_world` from `DragMove` (existing behavior)
- `ScreenSpace1DSlider`
  - meaning: consume optional screen delta (pixels) from `DragMove` and map to a 1D scalar
  - intended primarily for rotate rings and (later) scale handles

Notes:
- The mapping decision is **per handle** (translate vs rotate vs scale), not per pointer.
- For non-screen pointers (XR hands), `ScreenSpace1DSlider` will often be inapplicable; we should define a fallback (e.g. treat it as `PlaneWorldDelta`, or provide a VR-specific mapping later).

## Spec: add optional screen coordinates to drag signals

To support a true screen-space 1D slider, the consumer needs cursor/pointer motion in screen space.
Today, drag events only carry `hit_point` and `delta_world`.

We can add these fields as **optional** so non-screen pointers can leave them `None`:

- `DragStart { ... , screen_pos_px: Option<(f32, f32)> }`
- `DragMove { ... , screen_pos_px: Option<(f32, f32)>, screen_delta_px: Option<(f32, f32)> }`

Where:
- `screen_pos_px` is the current cursor position in pixels (window/viewport coordinates)
- `screen_delta_px` is the pixel delta since the previous `DragMove` for this drag

This is deliberately minimal:
- it doesn’t force a full “pointer system” refactor yet
- it gives gizmo handle mappings enough information to implement the 1D slider path

## How it would be used (with current routing)

With current routing, the easiest path is:

1. `GestureSystem` continues to emit `DragMove.delta_world` using plane projection (good for translation).
2. `GestureSystem` also emits optional `screen_delta_px`.
3. `GizmoSystem` resolves the active gizmo op (translate vs rotate vs scale) and also resolves `GestureCoordTypeComponent` by walking up ancestry from the hit renderable.
4. For rotate handles:
   - if `GestureCoordType` is `ScreenSpace1DSlider` and `screen_delta_px` is present: compute angle delta from screen delta
   - else: fall back to the current hit-point-based angle computation

This avoids rerouting gesture recognition right away.

## Rotation slider: consistent sign (high-level math)

We want “move mouse right -> positive rotation” (or similar), consistently, regardless of which side of the ring you clicked.

One robust way to do that is to:

- let `a` be the world-space rotation axis (unit vector)
- let `f` be the camera forward direction (unit vector)
- define a stable “increase rotation” world direction:
  - $t = \mathrm{normalize}(a \times f)$
  - (if $|a \times f|$ is near zero because we’re looking along the axis, fall back to $a \times r$ where $r$ is camera right)

Then convert screen delta (pixels) to a world-ish direction using camera basis:
- `dx, dy` in pixels (note: screen y is typically down)
- `r` = camera right (world)
- `u` = camera up (world)
- $m = r\,dx + u\,(-dy)$

Finally:
- scalar motion $s = \langle m, t \rangle$
- angle delta $\Delta\theta = k\,s$ (with sensitivity constant $k$)

This produces a consistent sign tied to camera orientation + axis, not tied to where on the ring the hit-point lies.

## Concrete examples (what we want)

### Translation gizmo axis

- Mapping: Start-plane projection (or axis-constrained plane projection) feels good.
- Continuation: camera pointer likely wants to continue while mouse-down, even off-target.

### Rotation ring

- Mapping: screen-space 1D slider is usually the best default.
  - e.g. horizontal mouse movement -> positive rotation
  - consistent sign regardless of which side of the ring you clicked
- Continuation: again, pointer policy decides whether the drag is allowed to continue when you drift.

### VR hand poke rotation (future)

- Mapping: might *not* be a ray at all; it may be a contact/overlap-based “grab and twist”.
- Continuation: likely requires contact (or a grip state).

This reinforces why “continuation policy” must be per pointer and not implicitly tied to raycasting.

## Architectural implication for the gesture system

The gesture pipeline should eventually look like:

1. Pointer lifecycle:
   - pointer emits button state (pressed/down/released)
   - pointer requests hits (ray hits, or contact hits)

2. Gesture system:
   - decides when a drag starts/ends for each pointer
   - captures a target + initial mapping context
   - enforces continuation policy (per pointer)

3. Gizmo/handle mapping:
   - given pointer updates, compute the delta (angle/translation/scale)
   - mapping choice comes from the handle/target

The key: the gesture system should not need to know “rotation should be screen-space 1D slider”; it only needs to:
- identify which handle/target is being dragged
- hold the drag state
- route pointer updates to the mapping logic chosen by the handle

## TODOs (migrated + updated)

This section is the updated successor to the prior v2 TODO list.

### A) Pointer-driven lifecycle (device-agnostic)

Goal: gestures should not read `InputState` directly for press/hold/release.

- [ ] Introduce pointer/button signals in `RxWorld` (names TBD), e.g.
  - `PointerButton { pointer_id, button, state: Pressed/Down/Released }`
  - or `PointerDown/PointerUp` + a per-pointer “is down” cache
- [ ] Make `GestureSystem` consume these pointer/button signals and maintain drag state per pointer.
  - This implies adding `pointer_id` to `DragStart/DragMove/DragEnd`.
- [ ] Update desktop user input to emit pointer/button signals for the mouse pointer.
- [ ] Update XR pipeline to emit pointer/button signals for controllers/hands (trigger/grab/contact).

### B) Pointer policy (continuation/lock) is per pointer

- [ ] Define a small enum for per-pointer continuation policy.
  - Examples: `RequireTargetContact` vs `AllowOffTargetContinuation`.
- [ ] Decide where it lives (likely on `PointerComponent` or a dedicated pointer config component).

### C) Handle mapping (coord type) is per handle

- [ ] Define per-handle mapping as `GestureCoordTypeComponent` (this doc’s spec).
- [ ] Add optional screen-space fields to drag signals (`screen_pos_px`, `screen_delta_px`) so handles can implement 1D slider mappings.
- [ ] Implement the true screen-space 1D slider first for rotation rings.
  - Keep the current hit-point-based rotation as a fallback.

### D) Raycast refactor follow-up (requests, not modes)

- [ ] Refactor raycasting to be request-driven (remove mouse-only gating inside raycast).
  - See: `docs/refactors/raycast-driven-by-actions.md`.

### E) Multi-pointer / arbitrary raycasters

- [ ] Teach gesture recognition to operate per pointer (not “global best hit across all raycasters”).
- [ ] Decide how `RequireTargetContact` generalizes beyond rays:
  - ray-based: require continuing `RayIntersected` against the captured target
  - contact-based: require continuing overlap/contact against the captured target

## Recommended next doc-level TODOs

- Define a small enum for **pointer continuation policy** (per pointer).
- Define a small enum for **handle mapping** (per handle) and encode it as `GestureCoordTypeComponent`.
- Add optional screen-space fields to drag signals (`screen_pos_px`, `screen_delta_px`) so handles can implement 1D slider mappings.
- When we implement the true screen-space slider, do it first for rotation ring.

