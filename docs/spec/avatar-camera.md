# Avatar Camera

How cameras are positioned relative to a humanoid avatar — both for XR first-person and
for desktop first-person (`Camera3D`).

---

## 1. Current XR Camera Setup

### Topology (vr-input example)

```
xr_input  (InputXRComponent)        ← receives HMD pose from OpenXRSystem
  └── xr_rig  (TransformComponent)  ← driven_t; OpenXRSystem writes HMD world TRS here
        ├── CXR                      ← XR camera marker; rig_world derived from xr_rig's parent world
        ├── CTLXR(Left, Aim) { ... }
        └── CTLXR(Right, Aim) { ... }

avatar_input_xr  (InputXRComponent) ← separate InputXR, also receives HMD pose
  └── avatar_driven_t  (TC)
        └── AVC
              ├── body_pipeline → model_root → GLTF
              ├── CTLXR(Left, Grip)
              └── CTLXR(Right, Grip)
```

These two `InputXR` components are independent; both receive the **same** HMD world
transform from `OpenXRSystem` each tick.

### How `xr_rig_origin_world` works

`OpenXRSystem::xr_rig_origin_world(world, visuals)` resolves the reference frame for
composing OpenXR poses:

1. Find `CameraXRComponent` (from `visuals.active_xr_camera()` or first enabled CXR).
2. Walk up to the nearest `InputXRComponent` ancestor.
3. Find the TC child of that `InputXRComponent` (`driven_transform`).
4. Return `transform_parent_world(world, driven_transform)` — the **parent's** world
   matrix of the driven_t, not the driven_t itself.

In `vr-input`, `xr_input` is a scene root, so `xr_rig_origin_world` returns identity
(floor origin).  OpenXR stage space already has Y-up from the physical floor, so:

```
eye_world = identity · mat4(hmd_pose_in_stage_space) = eye at physical head height
```

The XR views are rendered correctly at the real eye position without any explicit height
offset in the ECS.

### Avatar height alignment

The avatar model is offset so its feet touch the floor:

```
avatar_driven_t.world_y  ≈  HMD_height_from_floor     (e.g. 1.75 m)
model_root.world_y       =  driven_t.world_y - AVATAR_HEIGHT_M   (e.g. 0.15 m)
J_Bip_C_Neck.world_y     ≈  model_root.world_y + neck_local_y    (e.g. 1.55 m)
CXR / eye_world_y        ≈  HMD_height_from_floor                (e.g. 1.75 m)
```

`AVATAR_HEIGHT_M` is currently a **hardcoded constant** (1.6 m in `vr-input.rs`).
If the constant does not match the avatar mesh's actual bone geometry, the neck bone
will not be at eye level — the camera will appear to float above the character's head,
or the neck will clip into the view.

**This is the root cause of the "camera seems low" issue**: AVATAR_HEIGHT_M = 1.6
gives a neck at ~1.4–1.5 m while the physical HMD (and therefore the XR camera) is at
the user's actual head height (commonly 1.7–1.85 m).

---

## 2. Desktop First-Person Camera (`Camera3D`)

No built-in first-person mode is implemented for desktop.

In `vtuber-desktop` and `vtuber-example`, `Camera3D` is on a third-person orbit rig
and does not follow any avatar bone.  A first-person desktop camera would need to:

1. Wait for GLTF to spawn (currently done procedurally with `tick_with_queue` +
   `process_commands`).
2. Find the camera anchor bone:
   ```rust
   universe.find_component(model_root, "[name='J_Bip_C_Head']")
   ```
3. Attach `Camera3DComponent` under that bone's **splice node** (the `TransformComponent`
   injected by `AvatarControlSystem`, stored as `AVC.splice_head`), not the raw bone.
   The splice TC carries the world rotation computed by AVC each tick, so the camera
   inherits the correct head orientation.

There is currently no declarative path to express this intent before AVC initialises —
it requires post-spawn procedural code.

---

## 3. Speculative Design

### Problem summary

Both problems have the same root: the camera and the avatar's bones are wired up
separately, so their positions can only agree if a hardcoded constant matches the mesh.

| Setup | Root cause | Missing piece |
|---|---|---|
| XR first-person | `AVATAR_HEIGHT_M` constant ≠ avatar mesh bone height | Derive model_root.y from actual bone geometry |
| Desktop first-person | No declarative path to anchor Camera3D to a bone pre-init | Topology discovery, like controllers |

Both are solved by the **same unified mechanism**: `camera_bone` on AVC, with auto-
calibration of `model_root.y` as a side effect of init.

---

### Unified design: `camera_bone` + topology discovery

#### AVC field

```rust
/// Bone to anchor discovered cameras to, and to calibrate model_root.y from.
/// Defaults to `head_bone` ("J_Bip_C_Neck") if not set.
/// Typically set one joint above head_bone: e.g. "J_Bip_C_Head".
pub camera_bone: Option<String>,
```

#### Init behaviour (`try_init_splices`)

During the same init pass where controllers and head splices are wired:

**Step 1 — Measure bone height and calibrate model_root.y**

Find the `camera_bone` (or `head_bone` as fallback) in the armature.  Read its local
Y position relative to `model_root` with the model at identity (pre-splice, pre-
pipeline — the GLTF rest pose):

```
camera_bone_local_y  =  world_position(camera_bone).y - model_root.world.y
                        (at init, model_root.world_y == driven_t.world_y == 0)
```

Emit `UpdateTransform(model_root, y = -camera_bone_local_y)`.

Effect:

```
model_root.world_y     = driven_t.world_y - camera_bone_local_y
camera_bone.world_y    = model_root.world_y + camera_bone_local_y
                       = driven_t.world_y                          ✓
```

The camera bone is now exactly at `driven_t`'s world position — which is the HMD
position in XR, or the player body origin in desktop.  No hardcoded constant.

**Step 2 — Discover cameras by topology**

Any `Camera3DComponent` or `CameraXRComponent` that is a **direct child** of AVC is
re-parented under the camera bone (or a splice TC at that bone) during init, in the
same pattern as controller discovery:

```
for each direct child of AVC:
    if Camera3D or CameraXR:
        emit_attach(camera_bone_id, camera_component)
```

The camera inherits the head bone's world transform (position + rotation), giving
correct first-person orientation for both desktop and XR.

#### Topology before and after init

```
// Before (declared in .mms / .rs):
avatar_input_xr
  └── driven_t
        └── AVC { head_bone: "J_Bip_C_Neck", camera_bone: "J_Bip_C_Head" }
              ├── T.with_position(0, 0, 0) { GLTF }    ← model_root; y will be calibrated
              ├── CTLXR(Left, Grip) { T {} }
              ├── CTLXR(Right, Grip) { T {} }
              └── CXR {}                                ← or C3D {}; will be re-parented

// After AVC init:
avatar_input_xr
  └── driven_t
        └── AVC
              ├── body_pipeline
              │     └── pipeline_output
              │           └── model_root  (y = -camera_bone_local_y, auto-calibrated)
              │                 └── GLTF
              │                       └── ...armature...
              │                             J_Bip_C_Neck_parent
              │                               └── splice_head (TC)     ← head rotation driver
              │                                     └── J_Bip_C_Neck
              │                                           └── J_Bip_C_Head
              │                                                 └── CXR  ← re-parented here
              ├── CTLXR(Left, Grip) re-parented to lower_arm
              └── CTLXR(Right, Grip) re-parented to lower_arm
```

#### Why XR views remain correct

`OpenXRSystem::xr_rig_origin_world` traces from `CXR` up to the nearest `InputXR`
ancestor (= `avatar_input_xr`, a root-level component), then returns the **parent's**
world of that InputXR's TC child — which is identity, the physical floor origin.

CXR's depth inside the armature tree has no effect on this computation.  OpenXR still
renders views as:

```
eye_world = floor_origin · mat4(hmd_stage_pose)  =  physical eye position
```

The only thing that changes is the avatar's head bone is now calibrated to also sit at
`driven_t.world_y = HMD world Y`, so the ECS skeleton and the XR view are coincident.

#### Desktop first-person

`Camera3D` re-parented under `J_Bip_C_Head` inherits the bone's world transform
directly.  Because `J_Bip_C_Head` is a child of `splice_head` (which AVC drives with
the computed head rotation), the camera automatically tracks head orientation each tick.
No procedural post-spawn code, no hardcoded height constant.

#### API sketch

```rust
// Rust:
AvatarControlComponent::new()
    .with_head_bone("J_Bip_C_Neck")
    .with_camera_bone("J_Bip_C_Head")   // also calibrates model_root.y
    .with_left_hand_bone("J_Bip_L_Hand")
    .with_right_hand_bone("J_Bip_R_Hand")
    .with_initial_yaw(std::f32::consts::PI)
    .with_hand_rotation_smoothing(220.0)
```

```
// .mms:
AVC {
    with_head_bone("J_Bip_C_Neck")
    with_camera_bone("J_Bip_C_Head")
    with_left_hand_bone("J_Bip_L_Hand")
    with_right_hand_bone("J_Bip_R_Hand")
    with_initial_yaw(3.14159)
    with_hand_rotation_smoothing(220.0)

    T { GLTF.new("assets/models/pc-rei.hoodie.glb") { EM.on() } }
    CTLXR.new(true, Left, Grip) { T {} }
    CTLXR.new(true, Right, Grip) { T {} }
    CXR {}     // or C3D {} for desktop; discovered and re-parented to J_Bip_C_Head
}
```

Note there is no longer a separate XR rig (`InputXR.on() { T { CXR {} } }`).  The
avatar `InputXR` drives both the body/head pose and, via `xr_rig_origin_world`, the
XR view rendering.  The Aim-pose controller debug cubes from the current `vr-input`
example would either be dropped or remain as a second `InputXR` block that has no CXR
(and therefore does not influence `xr_rig_origin_world`).

---

### Open questions / tradeoffs

- **Camera bone splice vs raw bone**: re-parenting directly under the raw `J_Bip_C_Head`
  bone is simpler (no extra splice TC).  A splice would only be needed if AVC also needs
  to *inject* rotation into that bone (currently it does not — only `J_Bip_C_Neck` is
  driven).  For now, re-parent directly under the raw bone.

- **`camera_bone` vs `head_bone` defaulting**: if `camera_bone` is `None`, fall back
  to `head_bone` for both camera placement and height calibration.  This preserves
  backward compatibility — existing AVC setups without `camera_bone` continue to work,
  and the auto-calibrated Y replaces the now-removed `model_root.with_position` constant.

- **Seated / offset scenarios**: if the user wants the model to stand at the floor but
  the camera at a different height (e.g. a sitting avatar in a standing-scale room),
  a `camera_y_offset: f32` field on AVC could shift the camera bone attachment point
  without affecting model_root calibration.  Out of scope for the initial implementation.
