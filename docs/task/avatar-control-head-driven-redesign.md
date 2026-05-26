# Avatar control: head-driven redesign

Tracking the rework of `AvatarControlComponent` (AVC) so the head bone — not
the neck — is the node that receives the HMD/Input world rotation, with the
spine bending underneath via FABRIK and the body yaw-following hips (rather
than the whole `model_root`).

## Problem statement

The previous AVC drove `J_Bip_C_Neck` directly from `driven_t` (HMD or desktop
Input world pose) via an `IKChain { AimConstraint }`. Because the entire torso
(neck → upper-chest → chest → spine → hips) is a rigid FK chain above the neck,
rotating the neck twisted the visible torso from the neck up. Camera + body
ergonomics were also tangled: `camera_bone` reparented cameras under a bone
that wasn't actually being driven 1:1 by the input, so first-person camera and
visible head pose could diverge.

## Target design

```
driven_t (HMD / Input world pose, 1:1) ──┬─→ head bone: world rotation (and position in VR)
                                          │
                                          └─→ yaw-follow → hips/body anchor rotation (threshold-gated)

spine FABRIK chain: hips → spine → chest → upper_chest → neck → HEAD (end-effector pinned to driven_t)
```

- Head bone receives the input pose directly (already in place via `AimConstraint`).
- Spine FABRIK chain bends between hips and head, so torso follows naturally
  when the head rotates.
- Body yaw-follow sinks at hips (not `model_root`), so the entire avatar
  doesn't rotate as one rigid block — only the hips swing, with FABRIK
  redistributing through the spine.
- Camera sits where the player's eyes are: in VR, `CameraXR` reads HMD pose
  directly (parent transform irrelevant); on desktop, `Camera3D` is reparented
  under the head bone with an eye-position offset.

## Done

### Step 1 — switch head_bone default from neck to head

- `src/engine/ecs/component/avatar_control.rs`
  - `head_bone` default `"J_Bip_C_Neck"` → `"J_Bip_C_Head"`
  - Docstrings updated; topology diagram updated.
- Updated `head_bone` strings in:
  - `examples/vtuber-desktop.mms`
  - `examples/vr-input.{rs,mms}`
  - `examples/bisket-bones-and-ik.mms`

Verified: pitching desktop input no longer twists the torso from the neck up.
Only the head bone rotates; spine stays still until a body yaw-follow kicks in.

### Step 2 — camera reparenting: accept `T { C3D }` wrapper

- `src/engine/ecs/system/avatar_control_system.rs`
  - Camera-children discovery now accepts both bare cameras (`AVC { C3D {} }`)
    and T-wrapped cameras (`AVC { T.position(0, 0.08, 0.07) { C3D {} } }`).
  - In the wrapped form, the T is the node reparented under `camera_bone`,
    preserving its local transform as the eye offset relative to the head
    bone pivot.

### Step 3b — AimConstraint copy_position (head bone tracks HMD translation)

In VR, physically pitching your head moves the HMD forward+down (your real head
pivots around your neck, so the HMD translates). OpenXR writes that translation
into `driven_t`. But `AimConstraint` was rotation-only — the head bone stayed
FK-pinned to the static neck pivot, so the avatar's head visibly swung around
the neck while the HMD/camera moved with the player's physical head. Position
divergence between HMD and head bone, visible in third person as a head "swing"
and in first person as the overlay-cube marker drifting away from the head.

- `src/engine/ecs/component/ik_chain.rs`
  - `IKSolver::AimConstraint { offset_yaw, copy_position }` — new
    `copy_position: bool` field.
  - When true, the joint's world position is also overridden to the target's
    world position (in addition to rotation).
- `src/engine/ecs/system/ik_system.rs`
  - `solve_aim` writes local translation from `inv(parent_world) * target_pos`
    when `copy_position` is set.
  - Other call site (test): defaults to `copy_position: false` (no behavior
    change for existing TwoBoneIK / rotation-only chains).
- `src/engine/ecs/system/avatar_control_system.rs`
  - AVC's head IK now uses `copy_position: true` — head bone fully tracks
    `driven_t` pose (position + rotation).
- `src/meow_meow/component_registry.rs`
  - `aim_constraint(offset_yaw, copy_position?)` — second arg optional.

Side effect: in third person, the head visibly detaches from the neck under
sharp pitch because the neck/spine don't bend yet. That's exactly what the
spine FABRIK chain (still to do) will solve — neck/upper_chest/chest bend to
follow the head's tracked position.

### Step 3a — eye-height calibration (`eye_height_from_head_bone`)

- `src/engine/ecs/component/avatar_control.rs`
  - New field `eye_height_from_head_bone: Option<f32>` + builder
    `.with_eye_height_from_head_bone(f32)`.
  - Round-trips through `to_mms_ast`.
- `src/engine/ecs/system/avatar_control_system.rs`
  - Calibration now does `model_root.y = -(head_bone_local_y + eye_offset)`
    when set, so the avatar's eye line (not the skull base) lands at
    `driven_t`'s world Y = HMD height.
- `src/meow_meow/component_registry.rs`
  - Wires the `eye_height_from_head_bone(...)` MMS call.
- `examples/bisket-vr-demo.mms` uses `eye_height_from_head_bone(0.08)`.

Note: this still leaves a residual face-poke when pitching hard, because the
head bone *pivot* is at the skull base. The mesh swings around that pivot
while the camera stays at the HMD eye position. The full fix is per-camera
mesh culling (see Known issues).

### Step 3 — desktop camera convention

In `examples/bisket-bones-and-ik.mms`:

```mms
AVC {
    head_bone("J_Bip_C_Head")
    camera_bone("J_Bip_C_Head")
    ...
    T.position(0.0, 0.08, 0.07).rotation(0.0, 3.14159, 0.0) {
        C3D {}
        Pointer {}
    }
}
```

- `position(0, 0.08, 0.07)`: eye offset relative to head bone pivot (Y up,
  +Z forward in head-bone local space).
- `rotation(0, π, 0)`: cameras render down -Z but avatar anatomical forward
  is +Z (VRM convention) — flip the camera 180° so its view direction
  matches the avatar's forward.
- `CameraXR` doesn't need the flip — OpenXR overrides pose anyway.

Verified: head + camera stay locked when pitching; view faces the direction
the avatar faces.

## To do

### Ergonomics
- [ ] Decide: should AVC auto-apply the 180° Y flip for `Camera3D` children
  (since it's always needed when parented to a VRM head bone), so users don't
  author it manually? Could be a `camera_flip_y(true)` opt-in/out on AVC.
- [ ] Add `eye_offset: [f32; 3]` field on AVC as a shortcut so the user
  doesn't always need to author a T wrapper for the eye offset.

### Body / spine FABRIK
- [ ] Implement `IKSolver::Fabrik` in `ik_system.rs` (currently declared in
  `ik_chain.rs` but no match arm).
- [ ] Add `BoneMappingSystem::resolve_spine_chain(model_root, "J_Bip_C_Head")`
  walking hips → spine → chest → upper_chest → neck → head.
- [ ] AVC: build the FABRIK chain at init when the spine chain resolves.
  Target = `driven_t`, end-effector = head bone, root pinned to hips.
- [ ] Move body yaw-follow sink from `model_root` to the hips bone TC.
- [ ] Add translation-follow for hips (xz-track `driven_t.xz` with lerp;
  Y stays grounded — foot IK later).

### Cleanup
- [ ] Remove `AvatarBodyYawComponent` + `AvatarBodyYawSystem` if unused — the
  yaw-follow is now done via the inline `QuatYawFollow` pipeline in AVC.

### Verification
- [x] Desktop pitching no longer twists torso (bisket-bones-and-ik)
- [x] Desktop camera locked to head pose with eye offset
- [x] VR (OpenXR) — head rotation matches HMD; body yaw-follows after threshold
  (verified via `examples/bisket-vr-demo`)
- [ ] VR — hand controllers (tracked + Grip + Aim) resolve and drive hands
- [ ] After FABRIK lands: torso bends naturally when looking up/down/around

### Known issues

**VR head-mesh visibility when pitching.** Same root cause as the (now-fixed)
desktop camera divergence: the head bone *pivot* sits at the skull base, while
the HMD pose sits at eye height. AVC currently calibrates `model_root.y` so
the head bone pivot lands at HMD Y — meaning the model's eyes/face mesh ends
up ~5-8cm *above* the HMD camera. Pitching swings the head mesh down into the
camera frustum, so the player sees the inside of the face/hair.

In desktop this was solved by wrapping the camera in a T with the eye offset
so the camera arcs *with* the face mesh. In VR, `CameraXR` pose is
hard-overridden by OpenXR — a T-wrapper offset can't move the rendered eye
position. Two paths:

1. **Per-camera mesh culling (proper fix).** Hide the avatar head mesh from
   the XR camera; show on third-person cameras. Requires a render-layer /
   visibility-mask system that does not currently exist
   (`src/engine/graphics`). Track separately.
2. **Recalibrate `model_root.y` to put the eyes (not the skull base) at HMD
   height.** Partial — face mesh still pokes in under sharp pitch, but better
   neutral alignment. Trivial change to AVC if `eye_offset_y` is known.

For the demo, `bisket-vr-demo.mms` includes a desktop overview camera
(`CameraTarget::Window`) positioned in front of the avatar so the operator
can see the rig from outside the headset while debugging.
