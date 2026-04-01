---

# Gestures and Transform Gizmos

For the broader pointer → trigger → ray → gesture flow, see `docs/spec/pointer-input-ray-gesture.md`.

This document is the up-to-date, code-matching description of the engine’s desktop interaction pipeline for **ray-hit driven drag gestures** and the **transform gizmo**.

At a high level:

- **RayCastSystem** answers “what is under the pointer?” by emitting `EventSignal::RayIntersected`.
- **GestureSystem** turns ray hits + input edges into a `DragStart`/`DragMove`/`DragEnd` stream.
- **EditorSystem** (optional) routes selection by reattaching the editor’s transform gizmo to the clicked target.
- **TransformGizmoSystem** interprets drags as TRS edits on a target `TransformComponent`.

The intent is to keep gestures as **system-owned state + signals** (not components), and keep the transform gizmo as a **component-driven visual subtree** whose renderables can be hit and mapped to an operation via ancestry.

## Quick status summary

Dragging is stable by default: `GestureSystem` defaults to `DragUpdatePolicy::StartPlaneProjection`, meaning that once a drag starts, it keeps producing `DragMove` deltas even if the pointer ray no longer intersects the thin handle geometry.

Picking is improved but still not perfect: broad-phase uses BVH AABB candidates, then a narrow-phase can reject candidates and continue (enabling basic “click through the hole in a ring” behaviors for supported shapes).

## Frame order (SystemWorld)

The relevant order in `SystemWorld::tick()` is:

1. `RayCastSystem` runs and pushes `EventSignal::RayIntersected` facts.
2. Signals are dispatched immediately (so immediate-mode handlers can run).
3. `GestureSystem` runs and pushes `EventSignal::DragStart` / `EventSignal::DragMove` / `EventSignal::DragEnd`.
4. Drag signals are dispatched immediately.
5. `TransformGizmoSystem` consumes drag signals, mutates transforms, and may emit follow-up signals.
6. Signals are dispatched immediately, then the command queue is flushed so transforms are visible this frame.

Immediate-mode Rx handlers are installed once at frame start; downstream systems can read `rx.signals()` during the frame without draining.

## Signals

All interaction facts here are `EventSignal` variants, while reparenting requests are expressed as `IntentValue`.

- `RayIntersected { raycaster, renderable, t, origin, dir }`
  - Fact emitted by `RayCastSystem`.
  - `t` is distance along `origin + dir * t`.

- `DragStart { raycaster, renderable, hit_point, screen_pos_px }`
  - Fact emitted by `GestureSystem` when left mouse is pressed and a ray hit exists.
  - `screen_pos_px` is `Option<(f32,f32)>` so non-screen pointers can omit it.

- `DragMove { raycaster, renderable, hit_point, delta_world, screen_pos_px, screen_delta_px }`
  - Fact emitted by `GestureSystem` while dragging.
  - `delta_world` is the world-space delta since the previous drag move.

- `DragEnd { raycaster, renderable, hit_point }`
  - Fact emitted by `GestureSystem` when left mouse is released.

- `Attach { parents, child }`
  - Intent emitted by `EditorSystem` (and others) to request reparenting.
  - Handled by intent execution.

- `ParentChanged { child, old_parent, new_parent }`
  - Fact emitted after topology changes during intent execution.
  - Consumed by `TransformGizmoSystem` to rebind its runtime target.

## Components (interaction-relevant)

### Ray casting

- `RayCastComponent`
  - Controls raycast mode and max distance.

- `PointerComponent`
  - Scene-facing pointer marker/config.
  - Current authored/runtime shape is “author `Pointer {}` and let it own/spawn the runtime `RayCastComponent`”.
  - Longer-term policy direction is tracked in [docs/draft/pointer.md](docs/draft/pointer.md): infer pointer behavior from pose lineage first, with a camera-anchored fallback for fixed-camera scenes.
  - A camera-local `Pointer` may remain attached under `Camera3D` / `CameraXR`; stronger outer driver ancestry should still win when gesture trigger policy is inferred.

- `RaycastableComponent`
  - Opt-in eligibility: if enabled in the renderable ancestry, the renderable is eligible for BVH insertion / ray hits.

- `RaycastableShapeComponent`
  - Optional override for narrow-phase hit testing.

### Gestures

- No `GestureComponent` exists by design.
- Drag state lives in `GestureSystem::state`.

### Editor routing

- `EditorComponent`
  - Marks an editor subtree root.
  - Holds a runtime cache of the editor’s `TransformGizmoComponent` id (not serialized).

### Transform gizmo

- `TransformGizmoComponent`
  - Attached under a target `TransformComponent` to make it manipulable.
  - On init it queues a registration command to spawn the gizmo visual subtree.
  - Runtime fields include `target_transform`, `active_raycaster`, and `visual_root`.
  - Has a `scale: f32` field to scale visuals independently of the target.

Handle markers (ancestors of clickable subtrees):

- `TransformGizmoTranslateComponent { axis }`
- `TransformGizmoRotateComponent { axis }`
- `TransformGizmoScaleComponent { axis }`

Per-handle mapping:

- `GestureCoordTypeComponent { coord_type }`
  - `GestureCoordType::WorldPlane` (plane / hit-point delta)
  - `GestureCoordType::ScreenSpace1DSlider` (pixel delta → scalar)
  - Current status: rotate handles are spawned with `ScreenSpace1DSlider` so rotation is driven by screen-space deltas.

### Rotation handle mapping

Rotation rings use screen-distance slider behavior rather than continuous world-plane/ring intersection.

Practical behavior:

- click a rotation ring
- drag anywhere on screen
- rotation continues from screen-space motion even if the cursor leaves the thin ring geometry

Current mapping rules:

- `DragMove.screen_delta_px` is the primary input for rotational handles in slider mode
- the slider produces an incremental angle each move
- translation handles continue using world-plane / projected world-space drag deltas

Why this is the default for rotation:

- ring/plane intersection is sensitive to camera angle and hit-point continuity
- rotation feels better when it behaves like a stable 1D screen-space dial
- this avoids common “flip” / sign-instability problems when dragging away from the ring itself

Fallback behavior:

- if screen-space cursor data is not available, slider-mode rotation does not currently provide a non-screen fallback by itself
- future non-screen pointers (XR/controller-driven rotation) will need their own explicit mapping/gesture path

## Systems (interaction-relevant)

### RayCastSystem

- Produces `RayIntersected` facts.
- Broad-phase uses BVH AABB candidates.
- Narrow-phase may reject the closest candidate and continue to the next, for supported shapes.

### GestureSystem

- Consumes ray hits and mouse button edges.
- Emits `DragStart/DragMove/DragEnd`.
- Default behavior is tuned for editor feel:
  - `drag_update_policy: StartPlaneProjection` by default.
  - This means drag continues even if the ray stops intersecting the original target.

### EditorSystem

- Installs an immediate-mode handler for `DragStart`.
- If the clicked renderable is under an `EditorComponent` and is not a gizmo handle:
  - Finds the nearest ancestor `TransformComponent` for the clicked renderable.
  - Resolves the editor’s `TransformGizmoComponent` (cached or DFS).
  - Emits `Attach { parents: [target_transform], child: transform_gizmo }`.

### TransformGizmoSystem

- Spawns transform gizmo visuals (translate arrows + rotate rings) on registration.
  - Note: `TransformGizmoScaleComponent` exists and scaling math is implemented, but scale handle visuals are not spawned by default yet.
- Resolves “what was grabbed” by walking up ancestry from the hit renderable:
  - finds the nearest handle marker component (translate/rotate/scale)
  - finds the owning `TransformGizmoComponent`
- Uses `active_raycaster` to ensure only the captured pointer drives the drag.
- On `ParentChanged` for a `TransformGizmoComponent`, rebinds `target_transform` to the new parent transform.

## Checklist / follow-ups

- ✅ Multi-candidate raycast results + narrow-phase rejection (enables basic line-of-sight behaviors for supported shapes)
- ✅ Rotation rings use `ScreenSpace1DSlider` rather than world-plane ring dragging
- ⬜ Input routing / capture (prevent camera look + gizmo drag fighting)
- ⬜ Local/world gizmo mode (and a concrete parenting/compensation strategy for world-mode)
- ⬜ Pointer-driven lifecycle (remove mouse assumptions; support per-pointer state, multi-pointer drags)
- ⬜ Request-driven raycast refactor (remove mouse gating inside raycast; see `docs/refactors/raycast-driven-by-actions.md`)
