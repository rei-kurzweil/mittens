# Input → Intent → Data Flow (UserInput, Raycast, Gestures, OpenXR)

For the consolidated pointer interaction pipeline, see `docs/spec/pointer-input-ray-gesture.md`.

This doc is a **wiring diagram** for how input currently flows through the engine, where “intent” lives, and how we can extend the same pipeline to XR controllers.

Scope:
- **Today (desktop):** `UserInput` → `InputState` → `RayCastSystem` → `GestureSystem` → `GizmoSystem`
- **Today (XR):** `OpenXRSystem` publishes XR **camera** matrices (HMD) into `VisualWorld`
- **Proposed (XR controllers):** a `ControllerXRComponent` that `OpenXRSystem` uses to drive controller transforms, and then controllers participate in raycast + gestures.

This is **docs only**; no implementation in this file.

---

## Key types and where they live

### “Raw input” (winit / desktop)

- `UserInput` and `InputState`: `src/engine/user_input.rs`
  - Owned by the window loop (`src/engine/windowing.rs`).
  - Converts winit `WindowEvent`s into a small per-frame snapshot:
    - key/button down + pressed/released edge sets
    - cursor position, wheel delta
    - derived mouse movement + derived drag state

Important: `InputState` is intentionally **window + cursor** oriented today. It has no concept of XR controllers.

### “Intent” (signals-first)

- Signals doc: `docs/signals.md`
  - **Actions** are intent (requests).
  - **Events** are facts (observations).

There is already an action for raycasts:
- `Action::raycast(raycaster)`: `src/engine/ecs/component/action.rs`
- This increments `RayCastComponent.cast_requests`, so raycasting can be driven by *non-mouse* sources too (animations, tools, future XR input).

### Picking / hit testing

- `RayCastComponent` (request/behavior): `src/engine/ecs/component/raycast.rs`
  - `mode`: `Continuous` or `EventDriven`
  - `max_distance`
  - `cast_requests` (runtime-only)

- `RayCastSystem`: `src/engine/ecs/system/raycast_system.rs`
  - Builds a ray and emits `EventSignal::RayIntersected { ... }` into `RxWorld`.
  - Uses `BvhSystem` broad-phase + narrow-phase filtering (for some shapes).

### Gestures

- `GestureSystem`: `src/engine/ecs/system/gesture_system.rs`
  - Reads `RxWorld::signals()` (without draining) and `InputState`.
  - Emits `DragStart`/`DragMove`/`DragEnd` (events) into the same `RxWorld`.

### Gizmos

- `GizmoSystem`: `src/engine/ecs/system/gizmo_system.rs`
  - Consumes `Drag*` events and queues transform updates.

### XR rendering + camera

- `OpenXRSystem`: `src/engine/ecs/system/openxr_system.rs`
  - Today it primarily:
    - Initializes OpenXR
    - Creates a session when Vulkan handles are available
    - During `render_xr(...)`, locates views and publishes **per-eye camera matrices** into `VisualWorld`.

- `CameraXRComponent`: `src/engine/ecs/component/camera_xr.rs`
  - Lets you choose which XR rig transform is “active” (used as the rig/world transform for views).

---

## Current desktop data flow (frame order)

The ordering below matters because systems depend on cached transforms and the BVH.

The high-level tick order is in `src/engine/ecs/system/system_world.rs`.

### 1) winit events → `UserInput`

- `Windowing::window_event(...)` feeds all `WindowEvent`s into `UserInput::handle_window_event`.
- On `RedrawRequested` it calls:
  - `user_input.start_frame()` (compute deltas)
  - `universe.update(dt, user_input.state())`
  - `universe.render()`
  - `user_input.end_frame()` (clear pressed/released edges)

### 2) `InputState` → systems

`SystemWorld::tick(...)` receives `input: &InputState` and runs systems.

Notable consumers:

- `InputSystem` (first): consumes `InputState` and may queue movement/camera/rig changes.
  - Recent policy: camera look uses **right-drag**, leaving left-drag for interaction.

- `TransformSystem` updates cached world matrices.

- `BvhSystem` builds/refits the broad-phase structure from world-space AABBs.

- `CameraSystem` updates active window camera selection.

- `OpenXRSystem` runs before raycast so XR camera selection is current.

### 3) `RayCastSystem`: cursor ray → `RayIntersected` events

- Ray computation is “cursor through active camera”:
  - Uses `VisualWorld::camera_view()`, `camera_proj()`, and `viewport()`.
  - Uses `InputState.cursor_pos` (defaults to screen center if missing).

- Casting frequency is controlled by `RayCastComponent.mode` and `cast_requests`.
  - `EventDriven` currently also casts while left is down and mouse is dragging; this was added so drags keep producing hits.

- Ray *source kind* is inferred from topology:
  - If the nearest ancestor `TransformComponent` has a camera child, ray source is cursor-through-camera.
  - Otherwise, ray source is **parent-forward** (-Z) from that transform’s world pose.

- Output:
  - Emits `EventSignal::RayIntersected { raycaster, renderable, t, origin, dir }`.

### 4) `GestureSystem`: ray hits + mouse state → `Drag*` events

- On left press:
  - If a `RayIntersected` exists, capture that `(raycaster, renderable)` as the active drag.
  - Emit `DragStart { hit_point }`.

- While left is down:
  - Emit `DragMove { hit_point, delta_world }`.

- On left release:
  - Emit `DragEnd { hit_point: Option<_> }`.

#### The “drag coordinate source” switch

`GestureSystem` currently owns:

```rust
pub enum DragUpdatePolicy {
  RequireTargetContact,
  StartPlaneProjection,
}
```

- `RequireTargetContact`: delta comes from consecutive ray-hit points; requires “still hitting the handle”.
- `StartPlaneProjection`: after `DragStart`, continue producing deltas by intersecting the **current cursor ray** against a captured plane; does *not* require continued handle hits.

This is why the switch ended up in `GestureSystem`: it is not “hardware input”, it is the **policy for converting raw pointer motion into a stable drag delta**.

### 5) `GizmoSystem`: drag events → queued transform updates

`GizmoSystem` consumes `Drag*` events and applies gizmo operations by enqueuing transform mutations. `SystemWorld` flushes the queue immediately so visuals update in the same frame.

---

## Why `DragUpdatePolicy` was placed in Gestures (not UserInput)

`UserInput`/`InputState` is currently a thin adapter from **winit** to a per-frame snapshot.

`DragUpdatePolicy` is not a property of the mouse device; it’s a property of the **gesture interpretation**:

- A mouse drag can be interpreted as:
  - “keep ray-hitting the same surface” (raycast coords), or
  - “screen-space delta projected onto a constraint” (screen-space coords)

Those choices are interaction semantics (and should eventually vary **per tool / per gizmo mode / per pointer type**), so keeping it in `GestureSystem` is reasonable.

That said, once we have XR controllers, we probably want this to become:

- a per-pointer setting (mouse pointer vs controller pointer), or
- a per-gesture setting (translate axis vs translate plane vs rotate ring)

…rather than a single global toggle.

---

## XR today: what exists and what’s missing

### Exists

- `OpenXRComponent` enables OpenXR initialization.
- `CameraXRComponent` identifies an XR rig transform for the HMD.
- `OpenXRSystem::render_xr(...)` publishes per-eye camera matrices into `VisualWorld`.

### Missing

- Any notion of **XR controller poses** in ECS.
- Any mapping from XR controller inputs (trigger/grip/buttons) into:
  - `InputState` (desktop snapshot), or
  - action/event signals (intent), or
  - gesture state.

So yes: the reason the current input pipeline “lives” in `UserInput` + `InputState` is that it is currently **winit-only**.

---

## Proposed: `ControllerXRComponent` (docs-level design)

Goal: represent controller devices in the ECS as transforms that can be:
- rendered (controller model)
- used as ray origins (pointer rays)
- used as interaction sources for gestures

### Minimal semantics

- Attach `ControllerXRComponent` under a `TransformComponent` that represents the controller root.
- `OpenXRSystem` tracks registered `ControllerXRComponent`s.
- Each XR frame (or engine tick), `OpenXRSystem` queries controller poses and **updates the transform** for that controller root.
- Everything parented under that transform automatically moves with it.

That matches your request: “update transforms nested under those components”. Practically, updating the controller’s root transform is the simplest way to update all nested nodes.

### Handedness / enumeration

OpenXR commonly models controllers as left/right via standard paths (`/user/hand/left`, `/user/hand/right`) and “subaction paths”. Some runtimes/devices can expose additional tracked controllers.

A flexible component shape is:

- Either explicit left/right:
  - `role = Left | Right`
- Or generic enumerated controllers:
  - `role = Any(u32)`

If you *only* care about “first controller / second controller”, `Any(0)`/`Any(1)` can work.

### Pose kinds (grip vs aim)

For interaction rays you often want an “aim” pose; for rendering/held-object attachment you often want a “grip” pose.

So the component should likely choose:
- `pose = Grip | Aim`

Even if we don’t implement both immediately, baking this into the design avoids repainting ourselves later.

### Coordinate space

Controllers should usually be applied in the same reference space as the HMD views (the `LOCAL` reference space already used in `OpenXRSystem`).

In engine terms:
- Controller pose from OpenXR is “space-from-controller” (or controller-from-space) relative to a reference space.
- We then compose with the active XR rig transform (the same rig used for the camera) to get world-space.

---

## How XR controllers would connect to RayCastSystem and Gestures

Once controller poses exist as transforms, the rest of the pipeline can stay largely the same.

### Controller raycasts

`RayCastSystem` already supports a forward ray when the raycaster is under a transform with **no camera child**:

- Attach a `RayCastComponent` under the controller transform.
- Ensure that transform does not also “look like a camera rig” (no camera child).
- Ray origin becomes the controller transform world position.
- Ray direction becomes controller forward (-Z) in world space.

This describes the current implementation.
The proposed scene-facing authoring cleanup is to make `Pointer {}` the authored component and let it own/spawn the runtime raycaster; see [docs/draft/pointer.md](docs/draft/pointer.md).

One important edge case is a fixed-camera scene with no separate pose-driver transform.
In that case the spec direction is:

- allow `Pointer` to be nested under `Camera3D` / `CameraXR`
- treat that camera as the pointer's pose anchor
- infer the trigger source from camera kind (`Camera3D` → mouse, `CameraXR` → dwell/confirm/runtime action)

So the generalized rule is not just “find a pose driver”, but “resolve pose lineage, preferring a real driver and falling back to a camera anchor when needed”.

There is one more refinement:

- `Pointer` may remain nested under the camera even if the whole camera subtree later becomes parented under `Input`, `InputXR`, or another driver lineage
- in that case the stronger outer driver ancestry should win for trigger inference
- the local camera attachment still describes the ray anchor / camera relationship

### “When to cast” for controllers

Right now `RayCastComponent::EventDriven` is effectively mouse-left-driven.

For XR we want:
- `Continuous` (always-on pointer), or
- event-driven by controller trigger.

The engine already has a non-mouse hook:
- `Action::raycast(raycaster)` increments `cast_requests`.

So a clean XR design is:
- `OpenXRSystem` (or a future XR input system) turns trigger presses into `Action::raycast(...)` for the controller’s raycaster.
- `RayCastSystem` stays device-agnostic.

### XR gestures

`GestureSystem` is currently mouse-only:
- start/end are tied to `MouseButton::Left`
- screen-space mode depends on `InputState.cursor_pos`

For XR controllers, we likely want **ray-based coordinates**:
- `DragUpdatePolicy::RequireTargetContact` (or a future “controller-space” source)

And we need a “pressed/held/released” concept for controller triggers.

Two reasonable evolutions:

1) **Add an XR input snapshot** (parallel to `InputState`)
   - e.g. `XrInputState { trigger_down/pressed/released, thumbstick axes, ... }`
   - `GestureSystem` consumes “a pointer state” which can be either mouse pointer or controller pointer.

2) **Use signals for controller input (intent)**
   - `OpenXRSystem` pushes action/event signals like:
     - `ControllerButtonPressed { controller, button }`
     - `ControllerButtonReleased { ... }`
   - `GestureSystem` becomes “signal driven” rather than `InputState` driven.

Option (2) fits the existing “signals-first” story, but option (1) can be simpler for per-frame analog axes.

---

## Suggested mental model going forward: Input providers → pointer state → intent

To unify desktop + XR without forcing `InputState` to become a huge device union, it helps to separate layers:

1) **Device adapters**
   - `UserInput` (winit) for keyboard/mouse
   - `OpenXRSystem` (OpenXR) for HMD + controllers

2) **Pointer state** (one per “pointer”)
   - Mouse cursor pointer
   - Left controller pointer
   - Right controller pointer

3) **Intent**
   - “start drag”, “continue drag”, “end drag”
   - “request raycast”

`GestureSystem` is a good place for (2→3): it already turns low-level inputs into higher-level drag events.

The missing piece is: today it only knows about “mouse pointer”. `ControllerXRComponent` gets us controller *poses*; then we can add controller *buttons* as either snapshots or signals to complete the loop.
