# Gestures + Gizmos v4 (current code status)

This doc is a **codebase-oriented snapshot** of how ray casting, gestures, and gizmos are currently wired together.

Related design notes:
- `docs/gizmo-and-gestures-3.md` (architecture split: pointer policy vs handle mapping)
- `docs/refactors/raycast-driven-by-actions.md`

## Big picture

At a high level, the pipeline is:

1. **RayCastSystem** casts rays and emits `EventSignal::RayIntersected` into `RxWorld`.
2. **GestureSystem** reads `RayIntersected` + `InputState` and emits `DragStart` / `DragMove` / `DragEnd`.
3. **GizmoSystem** reads the drag signals and mutates the target `TransformComponent` (translate/rotate/scale).
4. Command queue flush applies transform changes so rendering reflects the drag **in the same frame**.

Key idea: **raycast is “what is under the pointer”**, gesture is **“turn ray hits + input edges into a drag stream”**, gizmo is **“interpret drag stream as TRS edits”**.

## Frame order (SystemWorld)

The relevant tick order lives in `src/engine/ecs/system/system_world.rs`.

In the current setup:

- `TransformSystem` and `BvhSystem` run before raycast so hit tests see current transforms.
- `RayCastSystem` runs before gestures so `RayIntersected` exists in `RxWorld`.
- `GestureSystem` runs before gizmos so drag signals exist.
- `GizmoSystem` runs and then the queue is flushed immediately so the drag is visible that frame.

Important nuance: `RxWorld` signals are **not drained** until `SystemWorld::process_commands`. Systems like Gesture/Gizmo read the *current frame’s* accumulated signals via `rx.signals()`.

## Signals (RxWorld)

Signals are defined in `src/engine/ecs/rx/signal.rs`, and the signal bus is `src/engine/ecs/rx/rx_world.rs`.

- `RxWorld::push(scope, value)` appends a `Signal { scope, value }`.
- `scope` is used for handler dispatch: handlers attached at `scope` or any ancestor scope will fire when drained.
- During a frame, systems can read `rx.signals()` (read-only snapshot).
- After the frame tick, `SystemWorld::process_commands` drains signals and dispatches handlers.

### Interaction-relevant events

- `EventSignal::RayIntersected { raycaster, renderable, t, origin, dir }`
- `EventSignal::DragStart { raycaster, renderable, hit_point, screen_pos_px }`
- `EventSignal::DragMove { raycaster, renderable, hit_point, delta_world, screen_pos_px, screen_delta_px }`
- `EventSignal::DragEnd { raycaster, renderable, hit_point }`

Notes:
- `screen_pos_px` and `screen_delta_px` are **optional** so non-screen pointers (e.g. XR controller rays) can omit them.

## Components and topology

### Raycasting components

- `RayCastComponent` (`src/engine/ecs/component/raycast.rs`)
  - `mode: RayCastMode::{Continuous, EventDriven}`
  - `max_distance`
  - `cast_requests` (set by `ActionSystem` when `Action::raycast(...)` runs)

- `PointerComponent` (`src/engine/ecs/component/pointer.rs`)
  - Opt-in marker/config for “this raycaster participates as a pointer”.
  - **Current status:** it exists and is attached in examples, but it is not yet used to filter or route gesture input.

- `RaycastableComponent` (`src/engine/ecs/component/raycastable.rs`)
  - Explicit opt-in: a renderable is raycastable if a `RaycastableComponent { enable: true }` exists either:
    - as a child of the renderable, or
    - on an ancestor in the component tree (nearest one wins).

- `RaycastableShapeComponent` (`src/engine/ecs/component/raycastable_shape.rs`)
  - Optional explicit shape override for narrow-phase picking.
  - If absent (or `InferFromBaseMesh`), the raycast system infers from the renderable’s base mesh.

### Gizmo components

- `GizmoComponent` (`src/engine/ecs/component/gizmo.rs`)
  - Attached under a `TransformComponent` to make that transform manipulable.
  - At init it queues `REGISTER_GIZMO`, which spawns the visual + pickable handle subtree.
  - Runtime fields:
    - `target_transform`: resolved transform being edited
    - `active_raycaster`: which raycaster is currently driving the drag (single-pointer)
    - `visual_root`: spawned gizmo subtree root

Handle markers (intended to be ancestors of the clickable subtrees):
- `GizmoTranslateComponent { axis }`
- `GizmoRotateComponent { axis }`
- `GizmoScaleComponent { axis }`

### Per-handle mapping

- `GestureCoordTypeComponent` (`src/engine/ecs/component/gesture_coord_type.rs`)
  - `coord_type: GestureCoordType::{WorldPlane, ScreenSpace1DSlider}`

**Current usage:**
- `GizmoSystem::register_gizmo` inserts `GestureCoordType::ScreenSpace1DSlider` under each rotate handle root.
- Translation and scale currently stay on the existing world-space delta mapping.

## Systems and dataflow

### 1) RayCastSystem

Code: `src/engine/ecs/system/raycast_system.rs`

Registration:
- `RayCastComponent::init` queues `REGISTER_RAYCAST`.
- On command queue flush, `SystemWorld::register_raycast` forwards to `RayCastSystem::register_raycast`.

Ray source inference:
- If the nearest ancestor `TransformComponent` also has a `Camera2DComponent` or `Camera3DComponent` child, the ray source is **cursor-through-active-camera**.
- Otherwise the ray source is **parent-forward** (casts along engine forward, which is `-Z` in local space).

Casting:
- Uses BVH first (`cast_against_renderables_bvh`), falls back to a brute-force scan over a maintained “eligible renderables” set.
- On hit, emits:
  - `rx.push(hit_renderable, EventSignal::RayIntersected { ... })`

Modes:
- `RayCastMode::Continuous` casts every tick.
- `RayCastMode::EventDriven` casts on mouse press edges, or when `cast_requests > 0` (requested by actions).

### 2) GestureSystem

Code: `src/engine/ecs/system/gesture_system.rs`

Inputs:
- `RxWorld` signals (reads `RayIntersected`)
- `InputState` (mouse edges + `cursor_pos`)
- `VisualWorld` camera matrices (only needed for plane projection mode)

Behavior:
- Picks the closest `RayIntersected` hit across raycasters for the frame.
- On left mouse press, starts a drag and emits `DragStart`.
- While dragging, emits `DragMove` based on the configured `DragUpdatePolicy`:

`DragUpdatePolicy::RequireTargetContact`
- Only emits `DragMove` while the ray still intersects the originally-captured renderable.

`DragUpdatePolicy::StartPlaneProjection`
- Captures a stable drag plane at `DragStart`.
- During drag, casts a cursor ray and intersects it with that plane.
- Produces `delta_world` from the projected plane points.

Screen-space fields:
- `screen_pos_px` is set from `input.cursor_pos`.
- `screen_delta_px` is computed from `last_cursor_pos` (when both are known).

### 3) GizmoSystem

Code: `src/engine/ecs/system/gizmo_system.rs`

Spawn / registration:
- `GizmoComponent::init` queues `REGISTER_GIZMO`.
- On flush, `SystemWorld::register_gizmo` calls `GizmoSystem::register_gizmo`.
- `register_gizmo` spawns the visual subtree under a `gizmo_root` transform.

Pickability:
- Each handle subtree includes a `RaycastableComponent` root node so descendants are raycast-eligible.

Resolving what was grabbed:
- `resolve_gizmo_op_for_renderable(world, renderable)` walks up ancestry from the hit renderable.
  - finds the nearest handle marker (`GizmoTranslateComponent` / `GizmoRotateComponent` / `GizmoScaleComponent`)
  - and the owning `GizmoComponent`

DragStart:
- Stores `active_raycaster` on the gizmo so only that pointer can drive the drag.
- Optionally spawns a debug visualization plane if `CAT_DEBUG_GIZMO_DRAG_PLANE` is set.

DragMove:
- Ignores drag moves not coming from the active raycaster.
- Applies mapping based on op:

Translate:
- Projects `delta_world` onto the selected axis and adds it to the target translation.

Scale:
- Uses axis-projected `delta_world` to adjust scale, clamped to a minimum.

Rotate:
- Checks for `GestureCoordTypeComponent` in the renderable’s ancestry.
  - If `ScreenSpace1DSlider` and `screen_delta_px` is present: uses a pixel→radians mapping.
  - Otherwise: falls back to the existing world-space “hit point moves around axis” rotation mapping.

DragEnd:
- Clears `active_raycaster`.
- Cleans up debug plane if enabled.

## Practical example topology

This is a typical desktop setup:

- Camera rig
  - `TransformComponent`
    - `Camera3DComponent`
    - `RayCastComponent` (cursor-through-camera)
      - `PointerComponent` (marker; currently informational)

- Target object
  - `TransformComponent`
    - `GizmoComponent`
      - `TransformComponent` named `gizmo_root`
        - `GizmoTranslateComponent(axis=X)`
          - `RaycastableComponent` (pick root)
            - renderables for the X arrow
        - `GizmoRotateComponent(axis=Z)`
          - `GestureCoordTypeComponent(ScreenSpace1DSlider)`
            - `RaycastableComponent` (pick root)
              - renderable ring

## Known gaps / intentional limitations

- `PointerComponent` exists but is not yet used by GestureSystem for pointer identity, continuation policy, or pointer selection.
- Gestures are currently “mouse-left-button drag” only. Non-mouse pointers would need:
  - a way to emit equivalent “pressed/down/released” edges into `InputState` or a parallel signal path, and
  - a gesture driver that does not rely on `cursor_pos`.
- The rotation slider mapping is deliberately a first pass (simple `radians_per_px`). The doc-level plan is to make sign selection camera-aware.
