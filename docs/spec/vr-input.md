# VR input: OpenXR pose sources and transform integration

This document describes how cat-engine integrates OpenXR controller and hand-tracking input into the ECS transform system.

**Scope:**
- Controller pose actions (`aim` / `grip`)
- Hand-tracking root poses
- How both sources are unified at the `TransformComponent` boundary
- Current limitations and future directions

**Not covered here:**
- Per-finger joint driving (see `docs/spec/hand-tracking-armature.md`)
- Button/trigger gameplay semantics
- Haptic feedback

---

## High-level overview

The engine's XR input flow has two main phases:

1. **Render phase** (`OpenXRSystem::render_xr`): sample time-sensitive poses at predicted display time
2. **Tick phase** (`OpenXRSystem::tick_with_queue`): apply poses to ECS transforms via the command queue

```
OpenXR runtime
  ↓
OpenXRSystem::render_xr()
  → sync actions
  → locate controller spaces at predicted_display_time
  → locate hand-root joint at predicted_display_time
  → cache valid poses
  ↓
OpenXRSystem::tick_with_queue()
  → find registered ControllerXRComponent nodes
  → resolve preferred pose source (hand-root or controller action)
  → update child TransformComponent
```

This separation ensures:
- Time-sensitive pose sampling happens at the right moment (predicted display time)
- ECS mutations happen in the normal tick, visible to all downstream systems
- Downstream systems (raycast, collision, animation) see controller transforms as normal transforms

---

## Components and registration

### `ControllerXRComponent`

A lightweight marker component that declares:
- `hand`: `Left` or `Right`
- `pose`: `Aim` or `Grip` (for controller actions)
- `enabled`: whether this controller should drive a transform

It does **not** store poses; it only identifies what pose stream the node wants.

Typical authoring shape:
```
ControllerXRComponent
  ↓
  TransformComponent (driven by OpenXR)
    ↓
    RenderableComponent / other children
```

### Registration and tracking

When a `ControllerXRComponent` is initialized, it queues a registration command that routes to `OpenXRSystem`, which maintains a set of active controller component IDs.

This gives a clean data ownership boundary:
- **ECS** owns components and topology
- **OpenXRSystem** owns OpenXR session and pose cache
- **Registration** creates the mapping between them

---

## Pose sampling (render phase)

### `OpenXRSystem::render_xr`

This is called during the render pass when `FrameWaiter` provides the `predicted_display_time`. Sampling at this time is critical—locating poses at an arbitrary "now" causes judder/latency.

Flow:

1. `FrameWaiter::wait()` → provides `predicted_display_time`
2. `session.locate_views(...)` at `predicted_display_time` (for per-eye cameras)
3. Sync controller actions and locate controller spaces:
   - `sync_actions(...)`
   - For each controller space: `Space::locate(reference_space, predicted_display_time)`
   - If pose is valid (`POSITION_VALID` and `ORIENTATION_VALID`), write to `controller_pose_cache`
4. Query hand-tracking (if available):
   - If `XR_EXT_hand_tracking` is supported, locate hand-root joint at `predicted_display_time`
   - Write to `hand_root_pose_cache` if valid

The caches store optional poses:
- left controller aim / grip
- right controller aim / grip
- left hand root
- right hand root

### OpenXR action setup (session init)

When an OpenXR session is created, `OpenXRSystem` initializes controller input (best-effort):

1. Create an `ActionSet`
2. Create pose actions: `aim_pose` and `grip_pose`
3. Create subaction paths: `/user/hand/left`, `/user/hand/right`
4. Create action spaces for left/right aim and grip
5. Attach the action set to the session
6. Suggest bindings for common interaction profiles

Notes:
- This is intentionally best-effort; runtimes may ignore unsupported profiles
- If controller input initialization fails, XR rendering continues; controller transforms simply don't update
- The engine should keep suggesting known controller profiles; runtime diagnostics remain important

---

## Pose application (tick phase)

### `OpenXRSystem::tick_with_queue`

For each registered `ControllerXRComponent`:

1. Lookup the component in the world; drop stale IDs
2. **Resolve the preferred pose source** (see precedence below)
3. Find a `TransformComponent` child of the `ControllerXRComponent`
   - If no transform child exists, nothing is updated
4. Convert the OpenXR pose into the correct transform space (see below)
5. Queue `UpdateTransform` so downstream systems see the updated transforms

### Pose precedence

The current precedence is intentionally simple:

1. Prefer the tracked hand root when hand tracking is available and valid
2. Otherwise fall back to the controller action pose (`Aim` or `Grip`)
3. Otherwise leave the target without a resolved pose for that frame

This gives one consistent transform-driving path for:
- Controller-backed interaction profiles (e.g., Meta Quest, HTC Vive)
- Hand-tracking-backed interaction
- Debug/visualization helpers

### Space conversion details

OpenXR poses from `Space::locate(reference_space, ...)` are expressed in the OpenXR reference space. In the engine, we want controller transforms to move with the XR camera rig. We:

1. **Compute the XR camera rig world matrix** using the same rig selection logic as `render_xr`:
   - Look for an active `CameraXRComponent` (from `visuals.active_xr_camera()`)
   - Use `TransformSystem::world_model(...)` to get its cached `matrix_world`

2. **Compose the controller pose under the rig:**
   ```
   world_from_controller = rig_world · mat4(pose)
   ```

3. **Convert to local space** (relative to the transform's parent):
   - Local translation: `local_trans = parent_world^-1 · world_trans`
   - Local rotation: `q_local = q_parent^-1 ⊗ q_world`
   - Scale is typically passed through unchanged

4. **Write to TransformComponent** and queue `queue_update_transform(...)`

This means the `ControllerXRComponent` acts like a pose source/driver node, and the child transform is the attachment point for visible geometry or interaction helpers.

### Tick ordering

`SystemWorld` flushes the command queue immediately after `tick_with_queue`, so updated controller transforms are visible to subsequent systems (raycast, gestures, collision) in the same frame.

---

## Current limitations and future directions

### Limitations

- Only pose actions are wired (aim/grip). No button/trigger actions yet.
- Transform rotation extraction from matrices is best-effort; assumes rigid transforms.
- No explicit smoothing/filtering at the OpenXR level.
- Per-finger hand-joint driving is not yet implemented (see `docs/spec/hand-tracking-armature.md`).

### Future directions

**Transform filtering / smoothing:**
- Future smoothing and follow behavior should build on the general **transform pipeline** described in `docs/spec/transform-pipeline.md`
- Rather than hardcoding smoothing into `OpenXRSystem`, use `TransformStreamSystem` operators like `QuatTemporalFilter` or `Vector3TemporalFilter`
- This keeps XR input focused on source acquisition and pose resolution

**Hand armature driving:**
- See `docs/spec/hand-tracking-armature.md` for driving entire joint hierarchies from OpenXR hand tracking

---

## Hand tracking: current state

The current hand-tracking path uses `XR_EXT_hand_tracking` as a raw joint input source, but reduces it to a single per-hand root pose for now.

**Selection policy:**
1. Use `WRIST` if position and orientation are both valid
2. Otherwise use `PALM` if position and orientation are both valid
3. Otherwise treat hand root as unavailable

**Important distinction:**
- A controller `grip` pose is an interaction/runtime-defined holding pose
- A hand `palm` or `wrist` pose is anatomical joint data from hand tracking

So current hand-root behavior is:
- "Use a stable tracked hand-root-ish pose to drive a transform"
- Not "pretend OpenXR hand tracking already gives a canonical grip pose"

For per-finger/per-joint hand tracking, see `docs/spec/hand-tracking-armature.md`.

---

## Code pointers

**OpenXR system:**
- `src/engine/ecs/system/openxr_system.rs` — controller/hand init, pose caching, transform application

**Controller component:**
- `src/engine/ecs/component/controller_xr.rs`

**Orchestration:**
- `src/engine/ecs/system/system_world.rs` — tick ordering and command flushing

**Transform system integration:**
- `src/engine/ecs/system/transform_system.rs` — world matrix propagation; used to find XR camera rig transform

**Transform stream system (for future filtering):**
- `src/engine/ecs/system/transform_stream_system.rs` — temporal operators and transform composition
- `docs/spec/transform-pipeline.md` — full design documentation
