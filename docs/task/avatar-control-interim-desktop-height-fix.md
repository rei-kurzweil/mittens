# Task: Interim AvatarControl Desktop Height Fix

Quick fix to prevent anatomical collapse (head-at-feet) in desktop examples
without requiring a full architectural overhaul of the pose driver handlers.

## Problem statement

In `vtuber-desktop.mms`, the avatar's head is at its feet because:
1.  `camera_bone` is not set, so no height calibration occurs.
2.  `model_root_local_y` defaults to `0.0`.
3.  The desktop `Input` driver sits at `y=0`.
4.  `HeadPoseBodyXzFollowSystem` forces the body to `driven_t.y + model_root_local_y = 0`.

## Interim Solution

Instead of a full handler strategy, apply a two-part "quick fix":

### 1. Auto-default `camera_bone`
In `AvatarControlSystem::try_init_splices`, if `camera_bone` is `None` but `head_bone` is `Some`, treat `head_bone` as the camera bone for the purpose of height calibration and camera re-parenting.

This ensures that the avatar height (approx. 1.6m) is always measured and stored in `model_root_local_y` (as `-1.6`).

### 2. Desktop Height Grounding
Ensure that for desktop drivers, the avatar remains grounded at world `y=0`.

Since desktop `Input` drivers usually sit at world `y=0` by default (unless specifically moved), the measured `model_root_local_y` of `-1.6` will correctly put the feet at `-1.6`.

To make the avatar appear at world `y=0`, the user must either:
- Set the `Input` driver's `T` child to a standing height (e.g. `T.position(0, 1.6, 0)`).
- OR: The system could detect desktop mode and apply the inverse offset to the `model_root` so it stays at world `y=0` even if the driver is at `y=0`.

However, the user noted that **"once we compensate for the y translation on the model root, the change sticks"**. This suggests that simply ensuring the calibration happens (via the `camera_bone` default) is the primary blocker.

## Verification Checklist

### Preparation
- [x] Clear 'clutter' in `examples/vtuber-desktop.mms` to verify height.
- [x] Update agent instructions (`CLAUDE.md`, `.github/copilot-instructions.md`) with SSH/Wayland GUI guidance.

### Desktop (Test Fallback)
- [x] `examples/vtuber-desktop.mms`: Removed explicit `camera_bone` and verified grounding (head on shoulders).
- [x] `examples/vtuber-desktop-first-person.mms`: Removed explicit `camera_bone` and verified fallback re-parenting.
- [x] `examples/bisket-bones-and-ik.mms`: Removed explicit `camera_bone` and verified fallback.

### VR/XR (Verify No Regressions)
- [ ] `examples/vr-input.mms` / `examples/vr-input.rs`
- [ ] `examples/bisket-vr-demo.mms` / `examples/bisket-vr-demo.rs`
- [ ] `examples/bisket-vr-debug.mms` / `examples/bisket-vr-debug.rs`

## Implementation Steps
...
1.  **Modify `AvatarControlSystem`**:
    - [x] If `avc.camera_bone` is `None`, use `avc.head_bone` as the selector for height measurement.
    - [x] Perform the `UpdateTransform` on `model_root` as usual.
    - [x] Update diagnostic logging.

2.  **Verify Examples**:
    - [x] Run `vtuber-desktop` (confirmed head on shoulders).
    - [x] Run `vtuber-desktop-first-person` (confirmed camera re-parented).

2.  **Verify `vtuber-desktop.mms`**:
    - With the fix, the head should be on the shoulders.
    - If the avatar is buried in the floor, update the MMS to set the `Input` driver height to `1.6`.

## Acceptance Criteria
- `vtuber-desktop.mms` (without explicit `camera_bone`) displays head on shoulders.
- No regression in `bisket-vr-demo.mms`.
