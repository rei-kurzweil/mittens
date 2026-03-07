# VR input (OpenXR → controller components → transforms)

This doc describes the current XR controller pose flow in **cat-engine**, from the OpenXR runtime to ECS transforms.

Scope:
- Focuses on **pose** (aim/grip) for left/right controllers.
- Does **not** yet cover button/trigger actions, haptics, or XR-driven gestures.

## High-level pipeline

1. **OpenXR runtime** provides per-frame predicted display time and controller poses.
2. `OpenXRSystem::render_xr(...)` syncs actions and **locates controller spaces** at the predicted display time, updating an internal **pose cache**.
3. During the ECS update tick, `SystemWorld` calls `OpenXRSystem::tick_with_queue(...)`.
4. `tick_with_queue` finds all registered `ControllerXRComponent`s, maps each to a cached pose (left/right × aim/grip), and queues `TransformComponent` updates so downstream systems see the new controller transforms.

The intent is:
- Do OpenXR *time-sensitive pose sampling* (predicted time) in `render_xr`.
- Do ECS *world mutation* (transform changes) in the normal tick, via `CommandQueue`.

## Components and registration

### `ControllerXRComponent`

A `ControllerXRComponent` is a lightweight marker/config component that declares:
- `hand`: `Left` or `Right`
- `pose`: `Aim` or `Grip`
- `enabled`: whether this controller source should drive a transform

It does **not** store the pose itself; it only identifies what pose stream the node wants.

Implementation:
- Component type: `ControllerXRComponent`
- Hand enum: `ControllerHand`
- Pose enum: `ControllerPoseKind`

### Registration and tracking

When a `ControllerXRComponent` is initialized, it queues a registration command. The engine routes that registration into `OpenXRSystem`, which tracks controller component IDs in a set.

This gives a clear data ownership boundary:
- ECS owns the components and topology.
- `OpenXRSystem` owns OpenXR state and pose cache.
- Registration just creates the mapping between the two.

## OpenXR action setup (session init)

When an OpenXR session is created, `OpenXRSystem` best-effort initializes controller pose input:

- Creates an `ActionSet`.
- Creates pose actions:
  - `aim_pose: Action<Posef>`
  - `grip_pose: Action<Posef>`
- Creates subaction paths:
  - `/user/hand/left`
  - `/user/hand/right`
- Creates action spaces:
  - left/right aim space
  - left/right grip space
- Attaches the action set to the session.
- Suggests bindings for several common interaction profiles.

Notes:
- This is intentionally **best-effort**: runtimes may ignore profiles they don’t support.
- If controller input init fails, XR rendering can still work; controller transforms simply won’t update.

## Where poses are sampled (render)

### `OpenXRSystem::render_xr`

`render_xr` is where we have OpenXR’s `predicted_display_time` for the current frame. That time matters: locating controller poses at an arbitrary “now” can create judder/latency.

Flow:

1. `FrameWaiter::wait()` → provides `predicted_display_time`.
2. `session.locate_views(...)` at `predicted_display_time` (for per-eye cameras).
3. If controller input exists:
   - `sync_actions(...)`
   - `Space::locate(reference_space, predicted_display_time)` for each controller space
   - If pose is valid (`POSITION_VALID` and `ORIENTATION_VALID`), write it into `controller_pose_cache`.

The cache stores four optional poses:
- left aim
- right aim
- left grip
- right grip

## Where transforms are applied (tick)

### Why a separate `tick_with_queue` exists

Mutating ECS/world state in `render_xr` would mix rendering and simulation responsibilities.
Instead:
- `render_xr` updates a cache.
- `tick_with_queue` reads the cache and queues ECS updates.

### `OpenXRSystem::tick_with_queue`

For each registered `ControllerXRComponent`:

1. Lookup `ControllerXRComponent` in the world; drop stale IDs.
2. Pick the correct cached pose based on `(hand, pose)`.
3. Find the **nearest ancestor** `TransformComponent` (controller components can be nested).
4. Convert the OpenXR pose into the correct transform space:

#### Space conversion details

OpenXR poses from `Space::locate(reference_space, ...)` are expressed in the OpenXR **reference space**.
In the engine, we want controller transforms to move with the XR camera rig (if present). So we:

- Compute a rig world matrix, using the same rig selection logic as `render_xr`:
  - `visuals.active_xr_camera()` or the first enabled `CameraXRComponent`
  - `TransformSystem::world_model(...)`
- Compose the controller pose under the rig:

$$\text{world\_from\_controller} = \text{rig\_world} \cdot \text{mat4(pose)}$$

Then we convert the desired world pose into a **local** pose relative to the transform’s parent (if any):
- local translation: multiply by parent world inverse
- local rotation: $q_{local} = q_{parent}^{-1} \otimes q_{world}$

Finally we write to the `TransformComponent` and queue `queue_update_transform(...)`.

### Tick ordering

`SystemWorld` flushes the command queue immediately after `tick_with_queue`, so the updated controller transforms are visible to subsequent systems (e.g. raycast/gestures) in the same frame.

## Current limitations / next steps

- Only pose actions are wired (aim/grip). No button/trigger actions yet.
- Transform rotation extraction from matrices is best-effort and assumes the rig/controller matrices are mostly rigid transforms.
- No explicit filtering/smoothing; raw runtime poses are applied.
- Controller-driven raycasts and gestures are not yet plumbed; the controller transforms exist so existing systems (like `RayCastSystem` with `ParentForward`) can be used once a raycast component is parented appropriately.

## Code pointers

- OpenXR controller init, caching, and application:
  - `OpenXRSystem` in `src/engine/ecs/system/openxr_system.rs`
- Controller component:
  - `ControllerXRComponent` in `src/engine/ecs/component/controller_xr.rs`
- Tick orchestration and command flushing:
  - `SystemWorld` in `src/engine/ecs/system/system_world.rs`
