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
              │                                 │     └── [sys] splice_head  (TC)
              │                                 │           ├── [sys] IKChain { AimConstraint }
              │                                 │           │         target:        driven_t
              │                                 │           │         end_effector:  splice_head
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
        (state owned by TransformPipelineSystem, keyed by stage path)
    MergeTRS  (translation + scale pass through unchanged)
  PipelineOutput
    model_root  (re-parented here; inherits shaped body yaw, no pitch/roll)
```

Stripping pitch and roll before `model_root` means the model Y offset (`-camera_bone_height`)
is only ever rotated by a pure-Y quaternion — feet cannot arc when looking up.

### Head IK detail

```
splice_head  (TC, placed under neck_parent by try_init_splices)
  IKChain { AimConstraint }
    target_id:    driven_t          // InputXR-driven TC (HMD world rotation)
    end_effector: splice_head       // root joint = end joint for AimConstraint
    offset_yaw:   π (VR) / 0.0 (desktop)
    weight:       1.0
```

`IKSystem` runs each tick after `AvatarControlSystem`. It reads `driven_t`'s world
rotation, post-multiplies by `rot_y(offset_yaw)`, cancels `neck_parent` world rotation,
and emits `UpdateTransform` to set `splice_head`'s local rotation. `J_Bip_C_Neck`
(displaced under `splice_head`) inherits the result.

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
   camera bone sits exactly at `driven_t`'s world Y (= HMD eye height in XR).
2. Any `Camera3DComponent` or `CameraXRComponent` direct children of AVC are re-parented
   under the camera bone so they inherit its world transform each tick.

`avatar_height` overrides step 1 if set; step 2 still uses `camera_bone`.

---

## Desktop vs VR

The only authoring differences:

| Field | Desktop | VR |
|---|---|---|
| Primary driver | `InputComponent` | `InputXRComponent` |
| `forward_plus_z` | `true` | `false` (default) |
| `initial_body_yaw` | `0.0` (default) | `π` |
| `head IK offset_yaw` | `0.0` (derived) | `π` (derived) |
| `ControllerXR` children | none | Left + Right Grip |
| `hand_rotation_smoothing` | — | `Some(220.0)` typical |

---

## AvatarControlSystem responsibilities

### Init phase (lazy — retries each tick until `splice_head` is set)

1. Find `model_root` (first TC child of AVC).
2. Find `head_bone` by name under `model_root`; **retry silently if GLTF not yet spawned**.
3. Inject `splice_head` TC under `neck_parent`; create `IKChain { AimConstraint }` under it.
4. Create body pipeline as child of AVC; re-parent `model_root` under its output.
5. For each configured hand bone with a controller:
   - Call `BoneMappingSystem::resolve_arm_chain` (topology, `min_bone_length = 0.03`).
   - **Arm IK mode**: create `IKChain { TwoBoneIK }` under `upper_arm`; bone stays in skeleton.
   - **Simple splice mode**: re-parent controller under bone's original parent; displace bone.
6. If `camera_bone` set: measure bone height; emit `UpdateTransform(model_root, y=-height)`;
   re-parent camera children under camera bone.
7. Store all runtime IDs on `AvatarControlComponent`; `splice_head` being `Some` stops retries.

### Tick (after init)

`AvatarControlSystem::tick` only re-attempts init until `splice_head` is live.
All per-frame pose work is handled by:
- `TransformPipelineSystem` — body pipeline (yaw follow) + hand smoothing pipeline
- `IKSystem` — head AimConstraint + arm TwoBoneIK

---

## Open questions / future work

1. **Pole direction body-local space** — world-space pole breaks when body rotates.
   `IKChainComponent` should have `pole_space: BodyLocal | World`; or AVC rotates the
   pole vector by `model_root` world rotation each tick.

2. **Side-specific pole directions** — currently `[0, -1, 0]` for both arms.
   Should be `[-1, -0.5, 0]` / `[1, -0.5, 0]` once body-local space is sorted.

3. **Hand rotation smoothing in arm IK mode** — `hand_rotation_smoothing` currently
   only applies to simple splice. Add `QuatTemporalFilter` on end-effector rotation.

4. **Spine IK** — FABRIK chain from hips to neck driven by head offset from body.
   Requires `TranslationFollow` pipeline op first (body XZ lags head XZ).

5. **VRM naming preset** — `BoneMappingSystem::vrm_names()` tier-1 resolver that fills
   all standard VRM bone names in one call, with topology fallback for missing bones.
