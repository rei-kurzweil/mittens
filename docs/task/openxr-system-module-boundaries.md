# OpenXR system module boundaries

Status: proposed follow-up. Split only on request or approval.

## Observation

`src/engine/ecs/system/openxr_system.rs` is about 4,950 lines. It mixes session
and instance setup, swapchain and Vulkan extension negotiation, frame timing,
head and hand pose driving, eye camera publication, floor correction and a
large test module. Some families already live beside it
(`openxr_tracked_frame.rs`, `openxr_avatar_free_pointer_tests.rs`,
`camera_projection`).

The head lock (2026-11-25, `xr_head_lock.rs`) was added as its own module
with one call from `drive_head_pose`, following that pattern.

## Proposed split

- Keep `OpenXRSystem` and its per-frame entry points in place.
- Move by responsibility, not per function:
  - session, instance and swapchain negotiation;
  - pose driving (`drive_head_pose`, hand driving, floor correction, tracking
    frames) together with `xr_head_lock.rs`;
  - eye camera publication with `camera_projection`;
  - tests next to the family they cover.
- Inventory the functions and their callers before agreeing on boundaries.

## Constraints and validation

The head, hand and eye paths run every XR frame. Keep them direct calls with
no new allocation, dynamic dispatch or extra transform propagation. Run the
OpenXR unit tests, the avatar-free pointer tests and a headset session before
and after; compare XR frame timing on the Focus 3 and the desktop headset.
