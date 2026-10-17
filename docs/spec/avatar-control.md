# AvatarControl — Spec

Date: 2026-03-23

## Goal

Planned default behavior (2026-10-16): [head/neck bend and bounded body follow](../task/xr-head-neck-bend-and-body-offset.md)
is intended to activate automatically for supported mapped humanoids under
either `Input` or `InputXR`, implemented through an AVC-generated
[typed value graph](../draft/declarative-vector-operator-components.md), existing
transform operators and explicit output bridges.
Both use the same per-rig follow threshold, calibrated to contain the full
rotation-generated head/body displacement. Desktop look remains inside that
range; XR measured positional excess can cause follow. Both retain geometric
neck closure. This is a proposal, not current runtime behavior; it supersedes
XR-only scope and separate follow-policy assumptions in earlier notes.

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
              body_yaw_threshold:      π/4
              body_yaw_rate:           3.0
              initial_body_yaw:        π                 // VR: π (model faces -Z)
              hand_rotation_smoothing: Some(220.0)       // None = no smoothing
              neck_pin_enabled:        true
            }
              ├── model_root  (TC — Y offset calibrated from resolved camera anchor)
              │     └── GLTFComponent → [armature]
              │           └── HumanoidBoneMap (optional; implicit Auto when omitted)
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
              ├── [sys] body_operator  (TransformOperatorComponent)
              │         └── ForkTRS
              │               ├── MapRotation
              │               │     └── QuatYawFollow { threshold, rate, initial_yaw }
              │               └── MergeTRS
              │         PipelineOutput
              │               └── model_root  (TC, y = -camera_anchor_height)
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

### Body operator detail

```
TOP.fork_trs().yaw_follow(body_yaw_threshold, body_yaw_rate)
  (input = driven_t world matrix, via nearest T ancestor)
  (state owned by TransformOperatorSystem, keyed by operator ID / operation path)
  model_root  (inherits shaped body yaw, no pitch/roll; translation and scale pass)

```

Stripping pitch and roll before `model_root` means the model Y offset (the negative camera-anchor height)
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

**Arm activation:** AVC consumes the owning GLTF's retained `HumanoidBoneMapReport` and
classifies each side independently. A side is eligible only when its upper-arm, lower-arm,
and hand slots validate and an enabled direct `ControllerXRComponent` child for that side has
a direct tracked `TransformComponent` child. AVC does not perform name lookup or topology
inference itself.

An ineligible side creates no target and no `TwoBoneIK` chain. Its mapped bones remain in
their ordinary FK/rest/animation pose. In particular, desktop AVC with no XR hand drivers does
not manufacture identity targets at world origin.

Eligibility is classified once during AVC initialization. Generated target IDs and IK chains
are the cached active mode; inactive sides leave their runtime IDs empty. Replacing the
GLTF/AVC subtree initializes and classifies a new instance. Dynamic driver or topology refresh
requires a future teardown/rebind operation so old runtime nodes can be removed safely before
classification runs again.

---

## Humanoid map and camera calibration

AVC requests the owning GLTF's retained `HumanoidBoneMapReport`. It waits until the required
head slot resolves before changing topology. An authored map overrides Auto inference; when no
map is authored, AVC requests an implicit Auto map. Each arm is enabled independently only when
its upper arm, lower arm, and hand resolve. Missing neck merely disables the neck rest pin.

For the resolved camera anchor:
1. On init, `model_root.y` is set from the anchor's rest/world height so the anchor sits at
   `driven_t`'s world Y.
2. Any `Camera3DComponent` or `CameraXRComponent` direct children of AVC are re-parented
   under the anchor so they inherit its world transform each tick.
3. `Camera3D` paths receive an extra local `Y = π` correction so the desktop render camera
   keeps the engine's `-Z` camera-forward convention while the visible head path preserves the
   authored avatar-facing basis. `CameraXR` does not receive this correction.

`avatar_height` overrides step 1 if set; step 2 still uses the resolved anchor. Resolution
prefers an explicit arbitrary transform, then a validated central eye/camera transform, then a
generated eye midpoint, and finally the head. A missing explicit selector remains diagnosed even
when a fallback keeps the camera operational.

---

## Shared vs Different

The runtime is now mostly shared across desktop and XR. The key distinction is:
the avatar/head topology and head-target convention are shared, while the primary pose
driver and final camera consumer are still different.

| Aspect | Shared between desktop and XR? | Desktop | XR |
|---|---|---|---|
| AVC init flow | Yes | Same `try_init_splices` path | Same `try_init_splices` path |
| Body operator topology | Yes | `body_operator -> model_root` | `body_operator -> model_root` |
| Visible head topology | Yes | `driven_t -> head_target -> head_bone` | `driven_t -> head_target -> head_bone` |
| Head-target convention | Yes | Shared XR-style default | Shared XR-style default |
| Default `head_target` yaw / head IK offset yaw | Yes | `π` | `π` |
| Head bone restore after reparent | Yes | restore authored `head_rest_rot`, zero translation, preserve scale | same |
| Camera-anchor auto-calibration | Yes | same `model_root.y` calibration from the resolved anchor | same |
| Camera discovery by topology | Yes | direct AVC camera child discovered and re-parented | same |
| Camera anchor parent | Yes | same mapped/generated anchor | same mapped/generated anchor |
| Primary pose driver | No | `InputComponent` | `InputXRComponent` |
| Hand/controller drivers | No | usually none | optional `ControllerXRComponent` children |
| Body-yaw authored override support | Yes | supported | supported |
| Default body-yaw convention | Yes | `forward_plus_z = false`, `initial_body_yaw = π` | `forward_plus_z = false`, `initial_body_yaw = π` |
| Final render camera component | No | `Camera3DComponent` | `CameraXRComponent` |
| Extra camera local yaw correction | No | local `Y = π` applied to `Camera3D` path | none |

### Practical reading of the table

What is the same:
- The head and body rigging topology created by AVC.
- The head-target math and authored eye-offset sign convention.
- The way the visible head bone is restored and mounted.
- The mapped camera-anchor calibration and camera discovery/reparent flow.

What is not the same:
- Desktop is driven by `InputComponent`; XR is driven by `InputXRComponent`.
- XR may also include controller/hand drivers.
- Desktop `Camera3D` gets a final local yaw correction because it consumes the parent
  transform directly as a mono render camera, while XR view matrices come from OpenXR's
  eye poses rather than from the `CameraXR` parent transform alone.

---

## AvatarControlSystem responsibilities

### Init phase (event-backed lazy handoff)

1. Find `model_root` and request the first skinned GLTF's retained humanoid report.
2. Wait without topology mutation until the mapped head validates.
3. Create `head_mount` beneath `driven_t`; displace the mapped head beneath it.
4. Create body pipeline as child of AVC; re-parent `model_root` under its output.
5. Classify each arm independently; create its IK target and chain only when the complete map
   and a usable direct XR hand target both validate.
6. Calibrate and attach cameras to the report's operational `camera_anchor` target.
7. Store all runtime IDs on `AvatarControlComponent`; `head_mount` being `Some` stops retries.

### Tick (after init)

`AvatarControlSystem::tick` only re-attempts init until `head_mount` is live.
All per-frame pose work is handled by:
- `TransformOperatorSystem` — body pipeline (yaw follow) + hand smoothing pipeline
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
   only applies to simple splice. Add `TOP.smooth("rotation", factor)` on end-effector rotation.

4. **Spine IK** — FABRIK chain from hips to neck driven by head offset from body.
   Requires `TranslationFollow` pipeline op first (body XZ lags head XZ).

5. **Convention presets** — MMS factories under `assets/components/humanoid_bone_maps/`
   provide shared VRoid naming and discoverable Bisket/PC-Rei presets.
