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

## Target design: PoseDriverHandler Strategy

Instead of scattered conditional logic, AVC will delegate its driver-specific behavior to an internal strategy. This encapsulates how the system applies the pose driver to the avatar's hierarchy.

### AvatarControlPoseDriverHandler Lifecycle
Each handler implements two primary lifecycle methods:

1.  **`handle_init(world, avc, emit)`**:
    - Performed once when the system discovers the AVC needs initialization.
    - Handles **Hierarchy Topology**: How the `head_bone` is integrated (Splice vs In-place).
    - Installs **IK Constraints / Transform Streams**: Configures the head `AimConstraint`, body pipeline, etc.
    
2.  **`handle_pose(world, avc, emit, dt)`**:
    - Performed every tick.
    - Figures out how to manipulate all the transforms it owns based on the pose driver's current state.
    - For VR: Implements the head-rotation-compensated body follow rule.
    - For Desktop: Ensures the head and body stay synchronized with the local driver's translation.

---

### 1. Eye-Driven Handler (VR/XR)
- **Driver:** `InputXRComponent`
- **`handle_init`**: **Rigid Splice**. Re-parents the `head_bone` to a child of the pose driver (`head_target`). Configures `AimConstraint` with `copy_position: true`.
- **`handle_pose`**: Executes the `HeadPoseBodyXzFollowSystem` rule (`HMD - R_h * v_local`) to translate the body under the head.

### 2. Body-Driven Handler (Desktop)
- **Driver:** `InputComponent`
- **`handle_init`**: **Soft/In-place Splice**. Keeps `head_bone` as a child of the neck. Injects `splice_head` in-place. Configures `AimConstraint` with `copy_position: false`.
- **`handle_pose`**: Likely a no-op or simple sync. The body inherits translation from the `Input` driver (locomotion) via standard parenting; the head inherits from the body.

---

## Implementation Steps

### Phase 1 — Driver Identification & Handler Selection
- Add `driver_kind: AvatarDriverKind` to `AvatarControlComponent`.
- Enum `AvatarDriverKind { VR, Desktop }`.
- In `AvatarControlSystem::try_init_splices`, identify the driver type by ancestor lookup.

### Phase 2 — Conditional Hierarchy Setup
- Update `try_init_splices` to use the `driver_kind` to decide:
    - Whether to emit an `Attach` intent moving the head to `head_target`.
    - How to configure the `AimConstraint` (copy position or not).

### Phase 3 — Conditional Tick Logic
- Update `HeadPoseBodyXzFollowSystem::tick_one` to short-circuit if `driver_kind == Desktop`.
- Ensure `QuatYawFollow` (in the body pipeline) still runs for both.

### Phase 4 — Example Verification
- Verify `examples/vtuber-desktop.mms` works without `camera_bone` calibration.
- Verify `examples/vtuber-desktop-first-person.mms` allows the camera to be carried by the avatar's head.
- Verify `bisket-vr-demo.mms` still works correctly in VR.

## Acceptance Criteria

- `vtuber-desktop.mms` displays the avatar at normal height (head on shoulders).
- `bisket-vr-demo.mms` still works correctly in VR.
- Head rotation follows input in both modes.
- Desktop body yaw follow (lag/threshold) still works.
- First-person desktop camera inherits avatar head motion (including any animations/IK).
