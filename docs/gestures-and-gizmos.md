# Gestures and Gizmos

This document describes the input→interaction pipeline for mouse-driven dragging and transform gizmos.

The intent is to keep **gestures** as *system-owned state + signals* (not components), and keep **gizmos** as a component-driven visual subtree whose *renderables* can be raycast-hit and then mapped to an operation via ancestry.

## Goals

- Provide a clean pipeline: **Input → Raycast → Gesture → Gizmo → Transform updates**.
- Support click-drag in a way that is compatible with the engine’s deferred signal dispatch.
- Emit drag events with a **world-space delta per tick**.
- Avoid a `GestureComponent` (gestures are global interaction state, not per-entity state).

## Terminology

- **InputState**: per-frame mouse/keyboard snapshot from `engine/user_input.rs`.
- **RayCastSystem**: emits `RayIntersected` signals when the cursor ray hits raycastables.
- **GestureSystem**: interprets low-level input + ray hits into higher-level drag signals.
- **GizmoComponent**: marks a transform as having a gizmo; the target transform is resolved via ancestry.
- **GizmoSystem**: consumes drag signals and mutates transforms (via queued transform updates).

## Data flow (high level)

1. `InputSystem` populates `InputState`.
2. `RayCastSystem` runs and pushes `EventSignal::RayIntersected { ... }` into `RxWorld`.
3. `GestureSystem` inspects the **queued signals** and `InputState` and pushes:
   - `DragStart`
   - `DragMove` (includes `delta_world`)
   - `DragEnd`
4. `GizmoSystem` reads `Drag*` events (still queued in `RxWorld`) and applies transform updates.
5. After `SystemWorld::tick`, `SystemWorld::process_commands` drains `RxWorld` and dispatches handlers.

### Why `RxWorld::signals()` exists

Signals are normally drained and dispatched later in `SystemWorld::process_commands`. Gestures and gizmos need to react **within the same tick** to `RayIntersected` without draining signals early.

So `RxWorld` provides a read-only `signals()` view, and systems can push additional signals during tick.

## Signals

Defined in `engine/ecs/rx/signal.rs`:

- `RayIntersected { raycaster, renderable, t, origin, dir }`
  - Emitted by `RayCastSystem`.
  - `t` is ray distance along `origin + dir * t`.

- `DragStart { raycaster, renderable, hit_point }`
  - Emitted by `GestureSystem` when left mouse is pressed and a hit exists.

- `DragMove { raycaster, renderable, hit_point, delta_world }`
  - Emitted by `GestureSystem` while dragging.
  - `delta_world` is the difference between consecutive hit points:

    $$\Delta = p_{cur} - p_{prev}$$

- `DragEnd { raycaster, renderable, hit_point: Option<[f32;3]> }`
  - Emitted by `GestureSystem` when left mouse is released.
  - `hit_point` is the last known hit point, if any.

## GestureSystem

Source: `engine/ecs/system/gesture_system.rs`

### GestureState

`GestureState` is owned by `GestureSystem` and mirrors the “interaction mode” similarly to how `InputState` mirrors hardware input.

Typical fields:

- `dragging: bool`
- `drag_raycaster: Option<ComponentId>`
- `drag_renderable: Option<ComponentId>`
- `last_hit_point: Option<[f32; 3]>`

### Drag capture rules (mouse-only, v1)

- On `MouseButton::Left` **pressed**:
  - If there is a `RayIntersected` hit, capture that `(raycaster, renderable)` as the active drag.
  - Emit `DragStart` scoped to the hit renderable.

- While left button remains **down**:
  - If the current frame’s best hit is still the captured `(raycaster, renderable)`, compute delta from `last_hit_point` and emit `DragMove`.
  - Update `last_hit_point`.

- On `MouseButton::Left` **released**:
  - Emit `DragEnd` for the captured renderable.
  - Clear capture.

### Note on “best hit”

`GestureSystem` currently selects the closest `RayIntersected` in `RxWorld::signals()` (lowest `t`).

If/when multiple pointers exist (XR controllers, multi-raycaster UI), this should evolve into:

- One `GestureState` per pointer/raycaster, or
- A routing layer that chooses which raycaster(s) participate in gestures.

## Raycast requirements for dragging

To keep hit points updated during a drag without forcing fully continuous raycasts, `RayCastMode::EventDriven` is extended to cast when:

- a cast is requested, OR
- left mouse is pressed, OR
- left mouse is down AND `input.mouse_dragging()` is true.

This ensures `RayIntersected` keeps being produced during the drag.

## Gizmos

### GizmoComponent

Source: `engine/ecs/component/gizmo.rs`

- Attach a `GizmoComponent` under a `TransformComponent` you want to manipulate.
- `GizmoSystem` will automatically register it and spawn the gizmo visual subtree.
- The target transform is resolved from ancestry at registration time (so gizmos work for joints/armatures):
  - Start at the component that has `GizmoComponent`.
  - Walk upward via `parent_of` until a `TransformComponent` is found.
  - That transform is the gizmo target.

Example component tree (conceptual):

- `Transform (object root)`
  - `GizmoComponent`
    - `Transform (gizmo visuals root)`
      - `GizmoTranslateComponent { axis: X|Y|Z }` (ancestor)
        - `Transform ...`
          - `Renderable (arrow parts)`
            - `Raycastable`
      - `GizmoRotateComponent { axis: X|Y|Z }` (ancestor)
        - `Transform ...`
          - `Renderable (ring)`
            - `Raycastable`

Notes:

- Translate + rotate handle visuals are spawned automatically today.
- Scale is supported by the TRS resolution logic, but scale handle visuals are not spawned by default yet.

### GizmoSystem (current behavior)

`GizmoSystem` consumes `DragStart/Move/End` and applies mutations.

Key design point: there is no mode switch and no “tagging” of renderables. Instead, each clickable
subtree is parented under a TRS handle component, so the operation can be derived by walking up the
component graph.

- On `DragStart`: record the active drag for that `raycaster`.
- On `DragMove`:
  - Start at the dragged `renderable`.
  - Walk upward until you find the nearest TRS handle component:
    `GizmoTranslateComponent` / `GizmoRotateComponent` / `GizmoScaleComponent`.
  - Keep walking upward until you find the owning `GizmoComponent`.
  - Apply the corresponding operation to the gizmo's resolved target transform.
- On `DragEnd`: clear active state.

Transform mutation should be done via `TransformComponent` setters that queue `queue_update_transform` (so `TransformSystem` propagation and dependent systems stay consistent).

### Operation mapping (v1)

- Translate: project `DragMove.delta_world` onto the handle axis and apply that scalar along the axis.
- Rotate: use `DragMove.hit_point` and an inferred previous hit (`hit_point - delta_world`) to compute a
  signed angle about the axis (in the plane orthogonal to the axis), then apply a quaternion delta.
- Scale: project `delta_world` onto the axis and add it to the corresponding scale component (clamped to a minimum).

## Tick ordering constraints

For same-frame response, the intended order is:

1. `RayCastSystem` (produces `RayIntersected`)
2. `GestureSystem` (consumes `RayIntersected`, produces `Drag*`)
3. `GizmoSystem` (consumes `Drag*`, queues transform updates)
4. `queue.flush(...)` as needed (so raycast/visuals see updated transforms if required)

Signals are still **dispatched to handlers** after tick in `SystemWorld::process_commands`.

## Future work

- Add plane-constrained dragging (e.g. drag on camera-facing plane) so translation feels stable when the hit point changes across curved surfaces.
- Improve rotation/scale behavior (constraints, snapping, better drag mapping).
- Add per-raycaster gesture state for XR.
- Add UI affordances (hover highlight, axis handles, snapping).
