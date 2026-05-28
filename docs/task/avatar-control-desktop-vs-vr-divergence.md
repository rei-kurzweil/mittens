# Task: AvatarControl Desktop vs VR Divergence

Fix the "head-at-feet" anatomical collapse in desktop examples by making
`AvatarControlSystem` (init) and `HeadPoseBodyXzFollowSystem` (tick)
distinguish between VR/XR and Desktop input drivers.

## Problem statement

The current `AvatarControl` (AVC) implementation uses a **Rigid Eye-Driven** model
derived from VR requirements:

1.  The head bone is spliced out of the armature and parented to the pose driver.
2.  The body is translated every tick to sit under the head bone.

While correct for VR (where the HMD is the world anchor), this causes
**anatomical collapse** on Desktop:

- The `Input` (Desktop) driver sits at world `y=0`.
- The head is moved to `y=0`.
- The body follow logic moves the model root to sit under the head at `y=0`.
- Result: Feet at 0, Head at 0.

## Target design

AVC should detect whether it is being driven by an `InputXRComponent` (VR) or
an `InputComponent` (Desktop) and diverge its behavior.

### 1. Eye-Driven Mode (VR/XR)
- **Driver:** `InputXRComponent`
- **Head Splicing:** Rigid (re-parent head bone to driver child `head_target`).
- **Head Constraint:** `AimConstraint { copy_position: true }` (or managed by FABRIK).
- **Body Follow:** `HeadPoseBodyXzFollowSystem` enabled.

### 2. Body-Driven Mode (Desktop)
- **Driver:** `InputComponent`
- **Head Splicing:** Soft (head bone stays in armature).
- **Head Constraint:** `AimConstraint { copy_position: false }`. Rotation only.
- **Body Follow:** `HeadPoseBodyXzFollowSystem` disabled.
- **Topological Integrity:** Head inherits translation from body; body inherits
  translation from `Input` driver via standard parenting.

## Implementation Steps

### Phase 1 — Driver Identification
- Add `is_xr: bool` (and optionally `is_initialized: bool`) to
  `AvatarControlComponent`.
- In `AvatarControlSystem::try_init_splices`, check for `InputXRComponent` vs
  `InputComponent` ancestors of the `driven_t` (AVC's parent).
- Store the result in `is_xr`.

### Phase 2 — Conditional Head Splicing
- In `try_init_splices`, if `!is_xr`:
    - Do NOT re-parent the `head_bone`.
    - Inject `splice_head` as the parent of `head_bone` *in-place* (keep its
      parent as the neck).
    - Configure the `IKChain { AimConstraint }` with `copy_position: false`.
    - Ensure `driven_t` still serves as the rotation target.

### Phase 3 — Conditional Body Follow
- Update `HeadPoseBodyXzFollowSystem::tick_one` to early-return if `!avc.is_xr`.
- This ensures the avatar stays at its grounded height on desktop.

### Phase 4 — Example Verification
- Verify `examples/vtuber-desktop.mms` works without `camera_bone` calibration.
- Verify `examples/vtuber-desktop-first-person.mms` allows the camera to be
  carried by the avatar's head.

## Acceptance Criteria

- `vtuber-desktop.mms` displays the avatar at normal height (head on shoulders).
- `bisket-vr-demo.mms` still works correctly in VR.
- Head rotation follows input in both modes.
- Desktop body yaw follow (lag/threshold) still works.
- First-person desktop camera inherits avatar head motion (including any animations/IK).
