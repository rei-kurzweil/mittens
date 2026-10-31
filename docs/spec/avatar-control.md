# AvatarControl — Spec

Date: 2026-03-23
Updated: 2026-10-28

## Goal

Native first slice (2026-10-17): [HeadNeckBodyConstraint](../task/xr-head-neck-bend-and-body-offset.md)
automatically replaces legacy body follow for supported mapped humanoids under
either `Input` or `InputXR`, independently of the proposed
[VMN facility](../draft/declarative-vector-operator-components.md).
Both Input and InputXR drive the same spliced head. InputXR supplies a measured pose;
desktop Input derives a connected head target from look orientation, locomotion and calibrated
geometry. The shared solver calculates body placement from that head target and the bent neck.
Input adaptation differs; skeleton topology and body closure do not. This slice follows positional
input immediately, with no lean allowance. The calibrated common threshold and bounded follow
remain planned. Desktop user feedback reports improved first-person body visibility after the
camera-mount/near-plane correction; broader headset acceptance and tuning remain pending.

AVC builders: `head_neck_body_enabled(bool)` selects native geometry versus the
legacy fallback. `neck_pitch(share, extension_radians, flexion_radians)` and
`neck_roll(share, lateral_radians)` tune tilt distribution. Initial values are
true, 0.5 shares, 15-degree extension, 25-degree flexion and 25-degree lateral
bend (angles supplied in radians). Neck tilt excludes up-axis yaw twist;
existing yaw follow still owns body heading. Missing/unsupported rigs and
mounted avatars use diagnosed legacy fallback; invalid tracking suspends solving.
Current body yaw retains its existing owner. Coordinated pose writes propagate
before arm IK. Capsule grounding and rendered XR eye pose remain separately owned.
Head/HMD offset calibration includes the mapped head-to-eye-anchor rest path,
the authored camera wrapper, and the displaced head's retained rest rotation/scale.
Headset feedback exposed the missing anchor term in the initial implementation;
eye/view alignment now has an integration check on mitten and Bisket.

A single coordination point that rigs a humanoid avatar for first-person control from a
primary body/head driver (`InputComponent` or `InputXRComponent`) plus any number of
auxiliary pose drivers (`ControllerXRComponent` for hands, etc.) — without the
input-broadcast problem that causes body rotation when only the head should move.

Native crouch first slice (2026-10-21): AVC binds hips and both mapped leg chains
and solves foot penetration against flat static collidable support through the
retained BVH. `crouch_enabled(bool)` defaults true; explicit foot-local landmarks
via `crouch_sole_offsets(left_vec3, right_vec3)` are required. Knee direction,
maximum flexion and support search tolerance use `crouch_knee_pole(vec3)`,
`crouch_knee_limit(radians)` and `crouch_support_margin(metres)` respectively.
Default policy is body-local +Z, 150 degrees and 0.05 model metres. Only thigh,
knee and foot poses are owned; tracking, hips/spine, capsule and locomotion stay
with their current owners. Mounted/tracking-invalid/unsupported avatars release
the solve. Initial headset feedback (2026-10-22) reports that crouching works
fairly well when XR floor height is calibrated correctly. The XR runtime and
[XR settings](xr-settings.md) own tracking-floor calibration; crouch IK consumes
the calibrated pose and collision support without correcting the tracking origin.
See [crouch IK](../task/avc-foot-penetration-crouch-ik.md) for rig calibration,
diagnostics, test coverage and remaining headset checks.

Procedural walking tuning uses an optional direct child, `AVC { WalkingIK {
stride(0.32) step_duration(0.22) step_height(0.07) step_scale(1.3) } }`.
The scale multiplies forward prediction, maximum foot travel and swing lift;
1.3 is the default even without an authored component, and 1.0 restores the prior
distance tuning. Knee/thigh angles follow the reachable foot goal. The child
replaces AVC's walking distance/duration settings; existing `walking_*` calls
still work without a child, and `walking_enabled(false)` remains a master opt-out.
See [procedural walking](../task/procedural-lower-body-walking-ik.md) for ranges,
ownership and frame-rate checks.

IK visualization uses AVC's shared `ik_debug` switch. Existing arm-chain visuals
are implemented; extending that switch to native crouch markers is planned.
Those markers will show predicted soles, desired support targets, solved soles
and correction/residual lines through shared IK debug drawing helpers.

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

For Input and InputXR, `AvatarControlSystem::try_init_pose_drivers` creates the runtime
head/body targets and optional arm chains. The head is displaced; the neck stays in the skeleton.

```text
Input or InputXR
  driven_t                         authoritative head driver
    head_mount [sys]               calibrated offset / desktop-derived head arc
      J_Bip_C_Head                  spliced, local translation zero
        mapped camera anchor
          authored camera wrapper
            C3D or CXR
    AVC
      body_operator [sys]           retained YawFollow, when enabled
        model_root
          GLTF / imported skeleton
            hips / spine / neck     head no longer inherits this hierarchy
            upper arm / forearm / hand
      ControllerXR (optional per hand)
        tracked target T
      TwoBoneIK chains [sys]        only for validated, present hand drivers
```

The camera wrapper is discovered from AVC before being attached to the mapped anchor. XR's
rendered view still uses the measured InputXR/OpenXR eye pose, not an extra wrapper offset.

### Body operator detail

```
TOP.fork_trs().yaw_follow(body_yaw_threshold, body_yaw_rate)
  (input = driven_t world matrix, via nearest T ancestor)
  (state owned by TransformOperatorSystem, keyed by operator ID / operation path)
  model_root  (inherits shaped body yaw, no pitch/roll; translation and scale pass)

```

Stripping pitch and roll before `model_root` means the model Y offset (the negative camera-anchor height)
is only ever rotated by a pure-Y quaternion — feet cannot arc when looking up.

**No separate AimConstraint for the head.** Both desktop and XR create a calibrated
`head_mount` beneath `driven_t` and reparent the head beneath it. Desktop target generation can
translate that mount along the connected neck arc; XR preserves measured head/view authority.
The native head/neck/body constraint coordinates neck bending and body placement.

### Head IK detail

```
driven_t  (Input/InputXR-driven TC)
  head_mount  (fixed eye/head offset and forward-axis correction)
    J_Bip_C_Head  (displaced from the armature)
```

The mount inherits `driven_t`. The native head/neck/body constraint owns its desktop arc and
coordinated body/neck outputs. `IKSystem` solves the optional arm TwoBoneIK chains; it does not
provide a second head/body pose writer.

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

AVC requests the owning GLTF's retained `HumanoidBoneMapReport`. A pose-driven AVC waits until
the required head slot resolves before creating its head splice. An authored map overrides Auto
inference; when no map is authored, AVC requests an implicit Auto map. Each arm is enabled
independently only when its map and actual hand driver validate.

For the resolved camera anchor:
1. On init, `model_root.y` is set from the anchor's immutable model-relative rest height so the anchor sits at
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

Declare a first-person C3D/CXR directly under AVC, optionally inside **one** direct Transform
wrapper carrying its eye offset. A later manual head attachment, or two nested wrappers, bypasses
that camera discovery/calibration path. For a close desktop first-person view use `C3D { near(0.01) }`;
the general C3D near default remains 0.1 m and can discard outer shirt/chest surfaces about 8 cm away.
See [avatar camera anchoring](avatar-camera.md) and the
[desktop/XR attachment contract](../task/avc-desktop-xr-camera-attachment-contract.md).

## Untracked avatars and shared lower-body setup

With no Input/InputXR ancestor, AVC binds the authored model and humanoid map, generates its
capsule, and registers the shared lower-body solver without head splicing, head-height model
placement or body-follow operators. NPC model transforms and skeleton hierarchy remain authored.
Capsule initialization explicitly propagates the generated proxy before contact consumes it.

Walking defaults on through `walking_enabled`; `.walking_enabled(false)` opts out. Eligible
calibrated AVCs observe actual supported capsule X/Z movement from Velocity, scripts, input or
ancestor transforms after contact resolution. Pose-driven avatars retain their input-readiness
checks; untracked avatars need no fake XR pose. Sole calibration and usable capsule/support are
still required. Mounted avatars and lost support release gait; teleport-like discontinuities reset
history. See [shared setup](../task/shared-avc-setup-and-untracked-walking.md) and
[procedural walking](../task/procedural-lower-body-walking-ik.md).

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
