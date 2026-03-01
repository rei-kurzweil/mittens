# Gizmos and Gestures (v2 notes + TODOs)

This doc is a re-contextualized follow-up to:
- `docs/gestures-and-gizmos.md`
- `docs/input-intent-data-flow.md`
- `docs/gesture-refactor.md`
- `docs/screen-space-gizmo-drag.md`
- `docs/refactors/raycast-driven-by-actions.md`

Goal: map out what we need to change so gestures + gizmos work cleanly with:
- multiple raycasters (mouse/camera, XR controllers, AI/autonomous)
- arbitrary “press/hold/release” sources (not hard-coded to mouse click)
- per-raycaster drag update policy

## Current state audit (as of today)

### Ray casting

- `RayCastComponent` exists and can be attached anywhere.
- `RayCastSystem::tick_with_queue` supports two inferred ray sources:
  - cursor through the active camera (`RaySourceKind::CursorThroughActiveCamera`)
  - parent-forward (-Z) (`RaySourceKind::ParentForward`)

However, `RayCastSystem::should_cast(...)` currently depends on **mouse input edges** (`InputState`) even for non-cursor ray sources.
That means an autonomous/VR raycaster cannot be purely “self-driven” unless it uses `cast_requests` every frame.

Related refactor proposal: `docs/refactors/raycast-driven-by-actions.md`.

### Gestures

- `GestureSystem` currently consumes `EventSignal::RayIntersected` and emits `DragStart/DragMove/DragEnd`.
- Drag update policy is a single global setting on `GestureSystem`:
  - `DragUpdatePolicy::RequireTargetContact`
  - `DragUpdatePolicy::StartPlaneProjection`

But:
- `GestureSystem` is still **mouse-only** for drag lifecycle:
  - drag starts on `input.mouse_pressed(Left)`
  - drag continues while `input.mouse_down(Left)`
  - drag ends on `input.mouse_released(Left)`
- `StartPlaneProjection` currently uses a **cursor ray** (`ray_from_cursor(...)`) even if the active drag was initiated by a non-cursor raycaster.

So the “gesture lifecycle” and the “pointer ray” are not yet truly per-pointer/per-raycaster.

### Gizmos

- Gizmo updates are driven by `DragMove { delta_world }`.
- Gizmo debug tools (e.g. drag plane visualization) currently infer drag-start rays by looking at `RayIntersected` near `DragStart`.

## What we want (design constraints)

1. **Gesture lifecycle must not depend on mouse click**
   - Gestures should be driven by generic “pointer button state” (down/held/up) coming from *any* device/system.

2. **Drag update policy should be per-raycaster (or per-pointer)**
   - Desktop mouse gizmo raycaster: default to `StartPlaneProjection`.
   - XR hand push / poke: default to `RequireTargetContact` (or equivalently “require contact” even if the contact comes from overlap/collision, not a ray).
   - Autonomous/AI raycasters: policy should be configurable.

3. **UserInput-based raycaster is special**
   - It is coupled to:
     - a cursor position
     - active camera projection
     - mouse buttons + modifiers
   - That doesn’t mean the rest of the architecture should assume those things exist.

## TODO list (gizmo + gesture v2)

### 1) DragUpdatePolicy (implemented, but not finished)

Status: `DragUpdatePolicy` exists and is used inside `GestureSystem`, but it’s global and still mouse/cursor-oriented.

TODOs:
- [ ] Make `DragUpdatePolicy` configurable per raycaster/pointer (not global).
  - Likely options:
    - add a small config component under each `RayCastComponent` (e.g. `PointerGestureConfigComponent { drag_update_policy, ... }`)
    - or extend `RayCastComponent` to include `drag_update_policy`
- [ ] Ensure `StartPlaneProjection` uses the **current pointer ray for the active raycaster**, not always the cursor ray.
- [ ] Decide whether `DragUpdatePolicy` should live in:
  - `GestureSystem` state per active drag
  - OR `RayCastComponent` / pointer config component

### 2) Device-agnostic drag lifecycle (remove mouse assumptions)

Goal: `GestureSystem` should not read `InputState` directly for press/hold/release.

TODOs:
- [ ] Introduce pointer/button signals in `RxWorld` (names TBD), e.g.
  - `PointerButton { pointer_id, button, state: Pressed/Down/Released }`
  - or `PointerDown/PointerUp` + a per-pointer “is down” cache
- [ ] Make `GestureSystem` consume these pointer/button signals and maintain drag state per pointer.
  - This implies adding `pointer_id` to `DragStart/DragMove/DragEnd`.
- [ ] Update `UserInput` pipeline to emit pointer/button signals for the mouse pointer.
- [ ] Update XR pipeline to emit pointer/button signals for controllers/hands (trigger/grab/contact).

### 3) Arbitrary raycasters initiating drags

We want drags created from arbitrary raycasters (not necessarily user/camera). That implies:

3.a) The overall mechanism shouldn’t depend on clicking or any specific input device.

3.b) We will want different drag update policies for different ray casters, and the UserInput-based raycaster is special.

TODOs:
- [ ] Define what a “pointer” is in-engine.

  Working definition:
  - A **raycaster** answers “what does this ray hit?”
  - A **pointer** answers “which interaction channel is doing the asking, and what is its state/policy?”
    - stable `pointer_id`
    - button state (pressed/down/released)
    - per-pointer drag update policy (and later: mapping parameters, modifiers, etc.)

  The key design goal is: **not every raycaster should automatically become a pointer**.
  A raycaster can be a pure query tool (AI probe, debug sensor) without participating in gesture lifecycle.

  Options for pointer identity:
  - option A (simplest): `pointer_id == raycaster ComponentId`
    - Pros: trivial mapping; fewer concepts.
    - Cons: makes it hard to have multiple pointers sharing a raycaster, and it conflates query vs interaction.
  - option B: separate pointer IDs, with a mapping component on raycasters
    - This matches what we’ll eventually want for multi-touch, split rays (aim vs grip), or multiple interaction modes.
  - option C (recommended): introduce an opt-in `PointerComponent` that *attaches to / references* a raycaster
    - `PointerComponent { raycaster: ComponentId, drag_update_policy, ... }`
    - `pointer_id` becomes the `PointerComponent`’s ComponentId (or the owning entity id)
    - This makes “what creates a pointer out of a raycaster?” explicit: **attach PointerComponent**.

  With option C:
  - `RayCastComponent` stays focused on spatial query configuration.
  - `PointerComponent` carries interaction semantics + policy.
  - We can still choose to treat “pointer_id == raycaster id” as a *temporary* shortcut during migration.
- [ ] Refactor `RayCastSystem::should_cast(...)` so that EventDriven casting doesn’t hard-code mouse.
  - E.g. `EventDriven` casts only when `cast_requests > 0`.
  - Mouse raycaster can increment `cast_requests` every frame while down.
  - XR raycaster can do the same based on its own button/contact state.
- [ ] Teach `GestureSystem` to pick the “best hit” **per pointer** (not a single global best across all raycasters), so two pointers can drag simultaneously.
- [ ] Decide how “RequireTargetContact” works for non-ray contact interactions:
  - ray-based: require continuing `RayIntersected` against the captured target
  - contact-based: require continuing overlap/contact against the captured target

## Open questions

- Should `RayIntersected` be re-keyed to a more general concept like `PointerHit`?
  - For VR hands, “contact hit” isn’t always a ray.
- Should `DragStart` carry mapping parameters explicitly?
  - For `StartPlaneProjection`: include `drag_plane_point_world` + `drag_plane_normal_world` in the signal.
  - This avoids consumers needing to infer plane/ray from separate signals.

## Implementation sketch (minimal path)

1. Add `PointerComponent` (opt-in) that references a raycaster and stores per-pointer policy.
  - Minimal fields: `raycaster: ComponentId`, `drag_update_policy: DragUpdatePolicy`.
  - `pointer_id` is the pointer component id (or owning entity id).
2. Add a `PointerButton` signal emitted by:
   - `UserInputSystem` (mouse)
   - `OpenXRSystem` (controllers/hands)
   - arbitrary systems (AI)
3. Add a small “pointer driving” step (could be inside `UserInputSystem`/`OpenXRSystem`, or a dedicated `PointerSystem`):
  - while a pointer wants hover/drag updates, request raycasts for its `raycaster` each tick (per docs/refactors/raycast-driven-by-actions.md)
4. Modify `GestureSystem`:
  - consume `PointerButton` + ray hits
  - maintain drag state per `pointer_id`
  - compute deltas using the pointer’s ray source (via the pointer’s raycaster)

That gets us: multi-pointer, device-agnostic drags, per-pointer policy, and a clear place for future handle-specific mappings.

