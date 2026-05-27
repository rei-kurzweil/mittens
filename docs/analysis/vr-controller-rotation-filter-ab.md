# VR controller rotation-filter A/B

Historical note: references below to `TransformPipelineOutput` describe the removed authored output marker. The current authored topology for this test is `TransformForkTRS` with the filtered cube attached directly under the fork root.

## Goal

We want a fast A/B test for whether quaternion rotation smoothing in the transform pipeline is
actually affecting `ControllerXRComponent`-driven poses.

The current concern is:

- vector3 temporal filtering appeared to help translation
- quaternion temporal filtering does not appear to visibly affect controller rotation
- we need to distinguish between:
  - transform pipeline wiring not being applied to XR controller poses
  - quaternion filter math/state not behaving as expected
  - the visual test setup making the filtered result hard to notice

## Current A/B toggle

`examples/vr-input.rs` now supports:

- `--xr-controller-rotation-filter`
- `--no-xr-controller-rotation-filter`

Usage:

```bash
cargo run --example vr-input -- --xr-controller-rotation-filter
cargo run --example vr-input -- --no-xr-controller-rotation-filter
```

The example prints which mode is active at startup.

## Current diagnostic override

For the current investigation, the implementation of `QuatTemporalFilter` has been temporarily
replaced with a **hard freeze**:

- first frame: capture the incoming quaternion
- later frames: keep returning that captured quaternion unchanged

This is intentionally much stronger than smoothing. The purpose is to answer a binary question:

- if the transform pipeline is active, controller rotation should appear obviously frozen in VR
- if controller rotation still appears live, the filtered path is not the one driving the visible
  result

## Scope of the toggle

This flag currently affects the **debug controller cubes** spawned by `spawn_controller_cube(...)`.

When enabled:

- `ControllerXR`
  - `Transform`
    - `TransformPipeline`
      - `TransformForkTRS`
      - `TransformMapRotation`
      - `QuatTemporalFilter`
      - `TransformPipelineOutput`
        - filtered visual cube

When disabled:

- `ControllerXR`
  - `Transform`
    - visual cube directly attached with **no transform pipeline**

This isolates whether the rotation pipeline path itself produces a visible difference on the same
controller source pose.

## Armature path wiring

The VTuber wrist-driving path now uses the same flag-controlled rotation pipeline idea.

When enabled, the inserted hand-driving branch is:

- `LowerArm`
  - `ControllerXR`
    - direct driven `Transform`
      - `TransformPipeline`
        - `TransformForkTRS`
        - `TransformMapRotation`
        - `QuatTemporalFilter`
        - `TransformPipelineOutput`
          - existing wrist transform subtree

When disabled, the wrist is attached directly under the driven transform child of `ControllerXR`.

This matters because `OpenXRSystem` only drives a **direct `TransformComponent` child** of
`ControllerXR`. The previous armature splice skipped that child transform, so the wrist path was
not actually testing the transform pipeline.

## What to compare in VR

Compare the two runs while rotating controllers quickly and with small wrist jitter:

1. `--xr-controller-rotation-filter`
2. `--no-xr-controller-rotation-filter`

Questions to answer:

- Do the debug cubes visibly freeze in orientation when filtering is enabled?
- Is the effect absent even during large, sharp orientation changes?
- Does startup logging from the existing quaternion debug instrumentation show non-zero lag and
  near-zero filtered-step change even when the visual result looks unchanged?

## Likely follow-up branches

If **no visible difference** exists between enabled/disabled:

- inspect whether `TransformPipelineOutput` is receiving the filtered quaternion path we expect
- validate quaternion temporal state updates frame-to-frame for controller-driven transforms
- compare raw vs filtered rotations directly in the pipeline debug logs
- verify the visible cube is definitely attached under the filtered output transform rather than an
  unfiltered sibling path

If **debug cubes freeze**, but VTuber wrists still do not:

- re-check the exact wrist-driving topology at runtime
- verify the wrist subtree is parented under `TransformPipelineOutput` rather than under the raw
  driven transform

## Related files

- `examples/vr-input.rs`
- `src/engine/ecs/system/transform_stream_system.rs`
- `src/engine/ecs/system/openxr_system.rs`