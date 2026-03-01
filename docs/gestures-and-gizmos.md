# Gestures and Gizmos

This document describes the input→interaction pipeline for mouse-driven dragging and transform gizmos.

The intent is to keep **gestures** as *system-owned state + signals* (not components), and keep **gizmos** as a component-driven visual subtree whose *renderables* can be raycast-hit and then mapped to an operation via ancestry.

## Goals

- Provide a clean pipeline: **Input → Raycast → Gesture → Gizmo → Transform updates**.
- Support click-drag in a way that is compatible with the engine’s deferred signal dispatch.
- Emit drag events with a **world-space delta per tick**.
- Avoid a `GestureComponent` (gestures are global interaction state, not per-entity state).

## Current limitations (verified in code)

These are practical limitations of the current pipeline that show up immediately once gizmos are present.

- Broad-phase picking is **AABB-based** (BVH candidates come from world-space axis-aligned AABBs).
  - We now do a **narrow-phase** per candidate for some shapes (ring annulus, box as OBB, cone proxy, triangle, tetrahedron), and can reject a “too-close” AABB candidate and continue to the next one.
  - This enables basic line-of-sight behaviors like “click through the hole in a ring” when the ring’s AABB overlaps the object behind it.
  - Broad-phase is still AABB-only, so rotated/complex meshes can still produce false positives that must be filtered by narrow-phase.
- Mouse drag is currently a **global gesture** derived from “any mouse button is down + cursor moved”.
  - `InputState::mouse_dragging()` is not button-specific.
  - `InputSystem` uses mouse drag to rotate rigs/cameras (yaw/pitch), so **left-dragging a gizmo can also rotate the camera** unless you add routing/capture.
- Dragging a gizmo handle currently only updates while the cursor ray continues to hit the handle geometry.
  - If the cursor leaves the ring/arm while the button is still down, `GestureSystem` stops emitting `DragMove`, so `GizmoSystem` stops applying updates.
  - This is a drag-capture / “continue even without hit” design gap.
- Gizmo TRS math uses **world axes** (X/Y/Z unit vectors), but gizmo visuals are parented under the target transform.
  - There is currently no explicit local/world space mode switch.
  - This becomes especially noticeable once we add more accurate narrow-phase picking (because interaction precision goes up).

## TODOs (updated)

- ☑️ 1. There’s no fine grained picking / line-of-sight selection.
  - Partially addressed: multi-candidate broad-phase + narrow-phase lets us do “click-through” for some primitives (notably `ring_2d`).
  - Still missing: general per-mesh fine picking for arbitrary geometry.
- ☑️ 2. Axis aligned bounding boxes aren’t enough.
  - Broad-phase BVH is still axis-aligned AABBs.
  - Partially addressed: we now do shape-aware narrow-phase (including treating `box` as an OBB by ray→local transform).
- ✅ 3. If an AABB/OBB wins broad-phase, we need to be able to reject it in narrow-phase and move on to other candidates behind it.
  - Implemented: BVH query returns multiple candidates; raycast iterates candidates until narrow-phase accepts.
  - Implemented narrow-phase coverage includes: `ring_2d` (used for `circle_2d`), `cone` (proxy), `triangle_2d`, `tetrahedron`, plus `box` and `quad_2d`.
- ⬜ 4. Input routing is needed (or a control scheme like right click for camera, left click for interaction).
- ⬜ 5. Gizmos need a local/world mode.
- ⬜ 6. World-mode gizmo parenting constraints need a concrete solution.
  - If world-mode exists and gizmo is parented under the target, we need detach/compensate logic so the gizmo stays neutral in world space.
- ⬜ 7. Drag capture should continue even if the cursor ray stops hitting the handle.
  - Desired: once a handle is captured on `DragStart`, it should keep producing `DragMove` while the button remains down.

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

### Proposed: drag update policies (desktop vs VR)

Right now, our drag events are implicitly “ray-hit-point-driven”: we keep raycasting and use the
hit point to produce `delta_world`. That makes dragging dependent on continuing to hit the handle
geometry.

For gizmos, we often want a *different* update policy once a handle is captured.
Desktop/mobile UIs typically feel better when the drag can continue after capture even if you’re
no longer intersecting the thin handle geometry, while VR “hand push / contact” interactions often
want to stop as soon as contact is lost.

Proposed enum (naming aligned with current code):

```rust
enum DragUpdatePolicy {
  RequireTargetContact,
  StartPlaneProjection,
}
```

Proposed signal change:

- Add a `drag_update_policy: DragUpdatePolicy` field to `DragStart`/`DragMove`/`DragEnd`.
- Keep `hit_point` in the signal, but interpret it as:
  - `RequireTargetContact`: updated every tick from the raycast hit.
  - `StartPlaneProjection`: updated by intersecting the current pointer ray against a captured drag-start plane.
- Add `delta_screen: [f32; 2]` (pixels or NDC; pick one and standardize) to `DragMove` so screen
  space mode has a first-class delta.

This directly addresses the “drag stops when you leave the handle” issue: once captured, screen
space drag continues even if the cursor ray no longer hits the handle.

Implementation status (now):

- `GestureSystem` has a `drag_update_policy` setting that switches between “require target contact”
  and “start-plane projection” dragging.
- Signals are unchanged for now (still emitting `DragMove { hit_point, delta_world }` only); we
  have **not** added `coord_source` or `delta_screen` fields yet.

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
- While left button remains **down**:
  - Behavior depends on `drag_update_policy`.
  - If `drag_update_policy == RequireTargetContact`:
    - If the current frame’s best hit is still the captured `(raycaster, renderable)`, compute delta from `last_hit_point` and emit `DragMove`.
    - Update `last_hit_point`.
  - If `drag_update_policy == StartPlaneProjection`:
    - Do **not** require that the current frame’s best ray hit is still the captured handle.
    - Instead, intersect the current pointer ray against the captured drag-start plane and emit `DragMove` every tick while the button is down.
    - Still capture and update `last_cursor_pos` each frame.

This explains the “drag only works while hovering the gizmo” behavior: when the cursor ray stops hitting the captured handle, the best hit is no longer the captured renderable, so no `DragMove` is emitted.

### What currently determines whether a gizmo updates its target

The end-to-end condition is:

1. `InputState` must indicate the left button is down.
2. `RayCastSystem` must emit a `RayIntersected` signal for the handle *this tick*.
3. `GestureSystem` must choose that handle as the best hit and consider it still captured.
4. Only then does `GestureSystem` emit `DragMove`.
5. `GizmoSystem` only applies mutations in response to `DragMove` (and resets on `DragEnd`).

So if (2) or (3) stops being true mid-drag, mutation stops even though (1) is still true.

### What we probably want instead (design note)

Once `DragStart` captures a handle, we likely want to keep producing `DragMove` while the button remains down, even if the ray no longer hits the handle geometry.

Screen-space dragging is one way to do that; another is to keep raycasting but against a derived
constraint instead of the handle geometry.

Typical options:

- Continue raycasting, but against a **derived constraint** (axis line, plane, or analytic ring plane) rather than the handle geometry.
- Or: keep using the regular cursor ray, but if there is no eligible hit, still compute motion relative to a persistent constraint (e.g. “drag on plane through initial hit point”).

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

## Input routing / capture (why it matters)

Right now, the engine does not have a routing/capture layer that can say:

- “while dragging a gizmo handle, the camera should not consume mouse-drag rotation”, or
- “right mouse is camera look; left mouse is interaction/picking”.

As a result, input consumers can fight:

- `InputSystem` interprets mouse drag as rig rotation.
- `GestureSystem` interprets left mouse drag as an interaction gesture driven by ray hits.

Two common directions:

1. **Control scheme**: gate camera look to `MouseButton::Right` (or Alt+Left) and reserve left for picking/dragging.
2. **Routing**: add an input-capture concept (e.g. “UI captured pointer this frame”), so only one subsystem consumes mouse drag.

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

## Gizmo local/world space mode (what’s missing)

Many editors support two modes:

- **Local mode**: axes follow the selected object’s orientation.
- **World mode**: axes stay aligned to the global frame.

Current behavior (verified in `GizmoSystem`):

- Operation math uses fixed **world axes** (`axis.unit_vec3()`).
- Gizmo visuals are spawned under the target’s component subtree, so they **inherit the target’s transform**.

This means we don’t cleanly support either classical mode:

- Visually it trends toward *local* (because it inherits parent rotation),
- but interaction math trends toward *world* (because axes are world unit vectors).

### If we add a world-mode gizmo, parenting becomes a real constraint

If a gizmo is parented under the object it modifies and we want **world mode**, we need a way for the gizmo visuals to *not* inherit the parent’s rotation/scale.

Common solutions:

- **Detach**: parent the gizmo under a neutral world-space node, but keep a reference to the target transform.
- **Compensate**: keep it parented, but apply an inverse parent rotation/scale to the gizmo root so its net world orientation remains fixed.

Both require a deliberate design choice because existing code assumes “gizmo lives under target transform” for convenience.

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
