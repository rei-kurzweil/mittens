# Bug: vtuber-desktop Head at Feet (Anatomical Collapse)

## Status
Investigated. Analysis complete. Setting `camera_bone("J_Bip_C_Head")` in the MMS confirmed to fix the anatomical collapse, but this is a workaround. A proper fix involves distinguishing between VR and Desktop input drivers.

## Symptom
In the `vtuber-desktop` example, the avatar's head is placed at its feet. The body appears "crushed" or anatomically collapsed at the ground level (world Y=0).

In contrast, `bisket-vr-demo` displays the avatar correctly (head on shoulders), even if the VR headset is not yet connected/present.

## Root Cause
The `AvatarControl` (AVC) system has recently transitioned to a "head-driven" model where the primary pose driver (`driven_t`) is treated as the eye position. The body (`model_root`) is then positioned relative to this head position using a calibrated height offset.

The anatomical collapse happens because AVC currently **unconditionally splices the head out of the armature** and moves it to the pose driver's world position.

1.  **Splicing Logic:** `AvatarControlSystem::try_init_splices` re-parents the `head_bone` to `head_target` (a child of the pose driver).
2.  **Desktop Driver Position:** In `vtuber-desktop`, the `Input` driver sits at `y=0`. The head bone is moved to `y=0`.
3.  **Body Follow:** `HeadPoseBodyXzFollowSystem` recomputes the `model_root` world position every tick to sit under the head bone. Since the head is at `y=0` and no calibration offset was provided, the body is also moved to `y=0`.
4.  **Result:** Feet are at `y=0`, head is at `y=0`. Collapse.

## The Correct Behavior: VR vs Desktop

The core issue is that AVC treats `Input` (Desktop) and `InputXR` (VR) identically, but they require different topological strategies.

### VR Mode (Eye-Driven / External Anchor)
- **Anchor:** The HMD (OpenXR) is the ground truth for head position.
- **Strategy:** Splice the head out of the armature and link it to the HMD. "Warp" the body (translate it) so the neck matches the HMD.
- **Body Follow:** `HeadPoseBodyXzFollowSystem` is MANDATORY to keep the body under the HMD.

### Desktop Mode (Body-Driven / Internal Anchor)
- **Anchor:** The Avatar's grounded position (locomotion) is the ground truth.
- **Strategy:** Do NOT splice the head out of the armature. Keep it as a child of the neck bone.
- **Head Rotation:** Use an `AimConstraint` with `copy_position: false` to drive the head's rotation from the input driver, while allowing it to inherit translation from the body.
- **Body Follow:** `HeadPoseBodyXzFollowSystem` should be DISABLED. The body should simply inherit translation from the `Input` driver via standard parenting.
- **Yaw Follow:** `QuatYawFollow` should still apply to the body so it turns with the mouse.

## Technical Implementation Plan

### 1. Identify Driver Type
`AvatarControlSystem` should detect if it is being driven by an `InputXRComponent` or an `InputComponent` during initialization and store this in an `is_xr` flag on `AvatarControlComponent`.

### 2. Conditional Head Splicing
In `try_init_splices`:
- If `is_xr`: Continue using the current "Rigid Splice" (re-parenting head bone to the driver).
- If `!is_xr`: Use a "Rotation-Only Splice". Inject `splice_head` in-place (under the neck), and use an `IKChain { AimConstraint }` with `copy_position: false` to drive its rotation from the driver.

### 3. Conditional Translation Follow
In `HeadPoseBodyXzFollowSystem::tick_one`:
- Early return if `avc.is_xr` is false.

## Verification with `vtuber-desktop-first-person.mms`
A new example `examples/vtuber-desktop-first-person.mms` has been created to test this "Body-Driven" mode. In this mode:
- The avatar stands at its authored height.
- The head rotates to follow the mouse.
- The camera is parented to the head bone, allowing for a true 1st-person experience where the camera is "carried" by the avatar rather than the avatar being "stretched" to fit the camera.
