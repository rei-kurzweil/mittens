# AvatarControl — Spec

Date: 2026-03-23

## Goal

A single coordination point that rigs a humanoid avatar for first-person control from a
primary body/head driver (`InputComponent` or `InputXRComponent`) plus any number of
auxiliary pose drivers (`ControllerXRComponent` for hands, etc.) — without the
input-broadcast problem that causes body rotation when only the head should move.

---

## Design principle — route all drivers through AvatarControlComponent

> **Every transform driver that moves this avatar's bones must be a child of (or otherwise
> routed through) `AvatarControlComponent`.**

`AvatarControlSystem` uses this to:
1. Splice each driver into the correct point of the armature during init.
2. Create transform pipelines and IK chain components that shape each driver's pose
   before it reaches the armature.

---

## Authored topology (what the user writes)

```
InputXR
  └── driven_t  (TC — written each tick by OpenXRSystem)
        └── AvatarControlComponent {
              head_bone:               "J_Bip_C_Neck"
              camera_bone:             "J_Bip_C_Head"   // optional
              left_hand_bone:          "J_Bip_L_Hand"   // optional
              right_hand_bone:         "J_Bip_R_Hand"   // optional
              left_upper_arm_bone:     None              // optional; topology if None
              left_lower_arm_bone:     None
              right_upper_arm_bone:    None
              right_lower_arm_bone:    None
              body_yaw_threshold:      π/4
              body_yaw_rate:           3.0
              initial_body_yaw:        π                 // VR: π (model faces -Z)
              hand_rotation_smoothing: Some(220.0)       // None = no smoothing
            }
              ├── model_root  (TC — Y offset auto-calibrated from camera_bone)
              │     └── GLTFComponent → [armature]
              ├── ControllerXR (Left,  Grip) { T {} }   // discovered by topology
              └── ControllerXR (Right, Grip) { T {} }
```

---

## Runtime topology (after AvatarControlSystem init)

All `[sys]` nodes are created programmatically by `AvatarControlSystem::try_init_splices`.

```
InputXR
  └── driven_t
        └── AvatarControlComponent
              │
              ├── [sys] body_pipeline  (TransformPipeline)
              │         └── ForkTRS
              │               ├── MapRotation
              │               │     └── QuatYawFollow { threshold, rate, initial_yaw }
              │               └── MergeTRS
              │         PipelineOutput
              │               └── model_root  (TC, y = -camera_bone_height)
              │                     └── GLTFComponent
              │                           └── [armature]
              │                                 │
              │                                 ├── neck_parent
              │                                 │     └── [sys] head_mount  (TC)
              │                                 │           ├── [sys] IKChain { AimConstraint }
              │                                 │           │         target:        driven_t
              │                                 │           │         end_effector:  head_mount
              │                                 │           │         offset_yaw:    π (VR) / 0 (desktop)
              │                                 │           └── J_Bip_C_Neck  (displaced here)
              │                                 │
              │                                 ├── J_Bip_L_UpperArm
              │                                 │     ├── [sys] IKChain { TwoBoneIK }
              │                                 │     │         target:        left_ctrl driven_t
              │                                 │     │         end_effector:  J_Bip_L_Hand
              │                                 │     │         pole:          [0, -1, 0]
              │                                 │     │         copy_end_rot:  true
              │                                 │     └── J_Bip_L_LowerArm
              │                                 │           └── J_Bip_L_Hand
              │                                 │
              │                                 └── J_Bip_R_UpperArm
              │                                       ├── [sys] IKChain { TwoBoneIK }
              │                                       │         target:        right_ctrl driven_t
              │                                       │         end_effector:  J_Bip_R_Hand
              │                                       │         pole:          [0, -1, 0]
              │                                       │         copy_end_rot:  true
              │                                       └── J_Bip_R_LowerArm
              │                                             └── J_Bip_R_Hand
              │
              ├── ControllerXR (Left,  Grip)   ← stays under AVC in arm IK mode
              │     └── left_ctrl driven_t      ← world pos set by OpenXRSystem each tick
              │
              └── ControllerXR (Right, Grip)
                    └── right_ctrl driven_t
```

### Body pipeline detail

```
TransformPipeline  (input = driven_t world matrix, via nearest TC ancestor)
  ForkTRS
    MapRotation
      QuatYawFollow { threshold: body_yaw_threshold, rate: body_yaw_rate }
        (state owned by TransformStreamSystem, keyed by stage path)
    MergeTRS  (translation + scale pass through unchanged)
  PipelineOutput
    model_root  (re-parented here; inherits shaped body yaw, no pitch/roll)
```

Stripping pitch and roll before `model_root` means the model Y offset (`-camera_bone_height`)
is only ever rotated by a pure-Y quaternion — feet cannot arc when looking up.

**No transform pipeline or AimConstraint for the head.** AVC creates a fixed
`head_mount` beneath `driven_t` and reparents the head bone beneath that mount.
It therefore inherits the input/HMD pose directly.

### Head IK detail

```
driven_t  (Input/InputXR-driven TC)
  head_mount  (fixed eye/head offset and forward-axis correction)
    J_Bip_C_Head  (displaced from the armature)
```

The mount inherits `driven_t` directly. `IKSystem` remains responsible for arm
TwoBoneIK and spine/body solving, but does not drive this head mount.

### Arm IK detail

```
J_Bip_L_UpperArm  (FK bone in skeleton)
  IKChain { TwoBoneIK }
    target_id:       left_ctrl driven_t    // controller world position = wrist target
    end_effector_id: J_Bip_L_Hand         // hand bone stays in FK skeleton (not displaced)
    pole_direction:  [0, -1, 0]           // world-space elbow hint (elbow-down default)
    copy_end_rot:    true                  // wrist rotation copied from controller
    weight:          1.0
  J_Bip_L_LowerArm
    J_Bip_L_Hand
```

`IKSystem` builds the chain as `[UpperArm, LowerArm, Hand]` (first TC child at each step,
then `end_effector_id` directly). Bone lengths are measured from FK world positions at tick
time (stable since only rotations are modified by IK). The controller stays under AVC —
`OpenXRSystem` correctly converts world→local regardless of parent hierarchy.

**Arm IK vs simple splice selection:** `BoneMappingSystem::resolve_arm_chain` is called
during `try_init_splices` for any hand bone that has a controller. If the arm chain resolves
(topology walk finds upper/lower arm ≥ 3 cm apart), arm IK mode is used and the hand bone
stays in the FK skeleton. If resolution fails, simple splice mode is used: controller
re-parented under bone's original parent, hand bone displaced under controller (with optional
smoothing pipeline).

### Simple splice mode (no arm IK / fallback)

Used when `BoneMappingSystem` cannot resolve the arm chain, or when no controller is present.

```
bone_original_parent  (e.g. J_Bip_L_LowerArm)
  ControllerXR (Left, Grip)
    controller_driven_t
      [sys] hand_pipeline  (TransformPipeline — only if hand_rotation_smoothing is Some)
        ForkTRS
          MapRotation
            QuatTemporalFilter { smoothing_factor }
          MergeTRS
        PipelineOutput
          [sys] smoothed_t  (TC)
            J_Bip_L_Hand  (displaced here)
      — OR, if smoothing is None —
      J_Bip_L_Hand  (displaced directly)
```

---

## Camera bone auto-calibration

When `camera_bone` is set:
1. On init, `model_root.y` is set to `-(camera_bone_world_y - model_root_world_y)` so the
   camera bone sits exactly at `driven_t`'s world Y.
2. Any `Camera3DComponent` or `CameraXRComponent` direct children of AVC are re-parented
   under the camera bone so they inherit its world transform each tick.
3. `Camera3D` paths receive an extra local `Y = π` correction so the desktop render camera
   keeps the engine's `-Z` camera-forward convention while the visible head path preserves the
   authored avatar-facing basis. `CameraXR` does not receive this correction.

`avatar_height` overrides step 1 if set; step 2 still uses `camera_bone`.

---

## Shared vs Different

The runtime is now mostly shared across desktop and XR. The key distinction is:
the avatar/head topology and head-target convention are shared, while the primary pose
driver and final camera consumer are still different.

| Aspect | Shared between desktop and XR? | Desktop | XR |
|---|---|---|---|
| AVC init flow | Yes | Same `try_init_splices` path | Same `try_init_splices` path |
| Body pipeline topology | Yes | `body_pipeline -> model_root` | `body_pipeline -> model_root` |
| Visible head topology | Yes | `driven_t -> head_target -> head_bone` | `driven_t -> head_target -> head_bone` |
| Head-target convention | Yes | Shared XR-style default | Shared XR-style default |
| Default `head_target` yaw / head IK offset yaw | Yes | `π` | `π` |
| Head bone restore after reparent | Yes | restore authored `head_rest_rot`, zero translation, preserve scale | same |
| Camera-bone auto-calibration | Yes | same `model_root.y` calibration from `camera_bone` | same |
| Camera discovery by topology | Yes | direct AVC camera child discovered and re-parented | same |
| Camera anchor parent | Yes | same `camera_bone` anchor | same `camera_bone` anchor |
| Primary pose driver | No | `InputComponent` | `InputXRComponent` |
| Hand/controller drivers | No | usually none | optional `ControllerXRComponent` children |
| Body-yaw authored override support | Yes | supported | supported |
| Default body-yaw convention | Yes | `forward_plus_z = false`, `initial_body_yaw = π` | `forward_plus_z = false`, `initial_body_yaw = π` |
| Final render camera component | No | `Camera3DComponent` | `CameraXRComponent` |
| Extra camera local yaw correction | No | local `Y = π` applied to `Camera3D` path | none |

### Practical reading of the table

What is the same:
- The avatar rigging topology created by AVC.
- The head-target math and authored eye-offset sign convention.
- The way the visible head bone is restored and mounted.
- The camera-bone calibration and camera discovery/reparent flow.

What is not the same:
- Desktop is driven by `InputComponent`; XR is driven by `InputXRComponent`.
- XR may also include controller/hand drivers.
- Desktop `Camera3D` gets a final local yaw correction because it consumes the parent
  transform directly as a mono render camera, while XR view matrices come from OpenXR's
  eye poses rather than from the `CameraXR` parent transform alone.

---

## AvatarControlSystem responsibilities

### Init phase (lazy — retries each tick until `head_mount` is set)

1. Find `model_root` (first TC child of AVC).
2. Find `head_bone` by name under `model_root`; **retry silently if GLTF not yet spawned**.
3. Create `head_mount` beneath `driven_t`; displace the head bone beneath it.
4. Create body pipeline as child of AVC; re-parent `model_root` under its output.
5. For each configured hand bone with a controller:
   - Call `BoneMappingSystem::resolve_arm_chain` (topology, `min_bone_length = 0.03`).
   - **Arm IK mode**: create `IKChain { TwoBoneIK }` under `upper_arm`; bone stays in skeleton.
   - **Simple splice mode**: re-parent controller under bone's original parent; displace bone.
6. If `camera_bone` set: measure bone height; emit `UpdateTransform(model_root, y=-height)`;
   re-parent camera children under camera bone.
7. Store all runtime IDs on `AvatarControlComponent`; `head_mount` being `Some` stops retries.

### Tick (after init)

`AvatarControlSystem::tick` only re-attempts init until `head_mount` is live.
All per-frame pose work is handled by:
- `TransformStreamSystem` — body pipeline (yaw follow) + hand smoothing pipeline
- `IKSystem` — arm TwoBoneIK and spine/body solving

---

## Resolved (see [ik-system.md](ik-system.md) for details)

1. **Pole direction body-local space** — world-space pole breaks when body rotates.
   `IKChainComponent` now has a `pub(crate) avc_id` field; when an ancestor AVC is found,
   the solver rotates the pole by the model root world rotation each tick.  Non-AVC
   `TwoBoneIK` chains keep world-space behaviour.

2. **Side-specific pole directions** — defaults are now semantically body-local:
   `[-1, 0, -1]` for left, `[1, 0, -1]` for right.  The solver transforms them to world
   space each tick, so the mirroring stays correct even as the body turns.

## Open questions / future work
   only applies to simple splice. Add `QuatTemporalFilter` on end-effector rotation.

4. **Spine IK** — FABRIK chain from hips to neck driven by head offset from body.
   Requires `TranslationFollow` pipeline op first (body XZ lags head XZ).

5. **VRM naming preset** — `BoneMappingSystem::vrm_names()` tier-1 resolver that fills
   all standard VRM bone names in one call, with topology fallback for missing bones.
