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

### `VRHandComponent`

A lightweight marker component that declares:
- `hand`: `Left` or `Right`
- `pose`: `Aim` or `Grip` (for controller actions)
- `enabled`: whether this controller should drive a transform

It does **not** store poses; it only identifies what pose stream the node wants.

Typical authored shape:
```
VRHand
  ↓
  TransformComponent (driven by OpenXR)
    ↓
    RenderableComponent / other children
```

### Registration and tracking

When a `VRHand` node is initialized, it queues a registration command that routes to `OpenXRSystem`, which maintains a set of active controller component IDs.

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
   - Locate joint arrays at `predicted_display_time` using `XR_EXT_hand_tracking`
   - When supported, request controller and unobstructed sources at tracker creation and chain
     `XrHandTrackingDataSourceStateEXT` into that same locate call
   - Cache the valid hand root and its data source together; clear them on tracking loss or errors

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

For each registered `VRHandComponent`:

1. Lookup the component in the world; drop stale IDs
2. **Resolve the preferred pose source** (see precedence below)
3. Find a `TransformComponent` child of the `VRHandComponent`
   - If no transform child exists, nothing is updated
4. Convert the OpenXR pose into the correct transform space (see below)
5. Queue `UpdateTransform` so downstream systems see the updated transforms

### Pose precedence

Selection is independent for each hand:

1. Prefer a valid anatomical hand root when `XR_EXT_hand_tracking_data_source` reports
   `UNOBSTRUCTED`, even if the device also exposes active controller actions
2. Otherwise prefer the active requested action pose (`Aim`, `Grip`, or `GripAim`)
3. Otherwise use a valid hand root as a fallback
4. Otherwise leave the target without a resolved pose for that frame

Controller-derived joints retain controller priority. Runtimes without the data-source
extension retain the same controller-first fallback. Interaction profile names and localized
device names do not decide this: a free-hand device can emulate a controller profile.
See [OpenXR hand data sources](https://registry.khronos.org/OpenXR/specs/1.1/html/xrspec.html#XR_EXT_hand_tracking_data_source).

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

This means the `VRHandComponent` acts like a pose source/driver node, and the child transform is the attachment point for visible geometry or interaction helpers.

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
- Future smoothing and follow behavior should build on the general **transform pipeline** described in `docs/spec/transform-operators.md`
- Rather than hardcoding smoothing into `OpenXRSystem`, use `TransformOperatorSystem` operators like `TOP.smooth("rotation", factor)` or `TOP.smooth("translation", factor)`
- This keeps XR input focused on source acquisition and pose resolution

**Hand armature driving:**
- See `docs/spec/hand-tracking-armature.md` for driving entire joint hierarchies from OpenXR hand tracking

---

## Hand tracking: current state

The current hand-tracking path uses `XR_EXT_hand_tracking` as a raw joint input source, but reduces it to a single per-hand root pose for now.

**Selection policy:**
1. Use `WRIST` position when valid, otherwise `PALM` position
2. Prefer valid runtime `PALM` orientation, otherwise `WRIST` orientation
3. Convert native joint axes into the canonical hand frame: preserve `-Z` forward and map
   canonical `+Y` to native `+X` for the left hand, native `-X` for the right hand
4. When neither native orientation is usable, synthesize `-Z` from
   `MIDDLE_METACARPAL → MIDDLE_PROXIMAL` and `+Y` from projected
   `LITTLE_PROXIMAL → INDEX_PROXIMAL`
5. Reject the root when no valid origin or orientation source remains

**Important distinction:**
- A controller `grip` pose is an interaction/runtime-defined holding pose
- A hand `palm` or `wrist` position is anatomical joint data from hand tracking

The runtime joint quaternion is not treated as a canonical Aim or Grip frame. OpenXR native
`+Y` points out of the back of the hand, while the retained avatar basis uses palm width for
canonical `+Y`. The per-side conversion above reconciles those conventions before AVC applies
the avatar's retained target-rest correction. See the
[OpenXR joint conventions](https://registry.khronos.org/OpenXR/specs/1.1/html/xrspec.html#convention-of-hand-joints).

Native orientation avoids inferring wrist rotation from knuckle geometry. The landmark fallback
uses the middle metacarpal rather than a curling finger segment, so closing the fingers does not
pitch the wrist target. Orientations are validated independently of positions; non-finite or
zero-length quaternions fall through to the next source.

For per-finger/per-joint hand tracking, see `docs/spec/hand-tracking-armature.md`.

---

## Code pointers

**OpenXR system:**
- `src/engine/ecs/system/openxr_system.rs` — controller/hand init, pose caching, transform application

**Hand pose component:**
- `src/engine/ecs/component/controller_xr.rs`

**Orchestration:**
- `src/engine/ecs/system/system_world.rs` — tick ordering and command flushing

**Transform system integration:**
- `src/engine/ecs/system/transform_system.rs` — world matrix propagation; used to find XR camera rig transform

**Transform stream system (for future filtering):**
- `src/engine/ecs/system/transform_operator.rs` — temporal operators and transform composition
- `docs/spec/transform-operators.md` — full design documentation
