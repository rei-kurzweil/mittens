# Mirror camera orientation and tracking are wrong

## Status

Open bug / regression note.

## Symptom

The current mirror rendering path has two visible camera bugs:

- the reflected image is still upside down
- the mirror camera appears fixed in place instead of tracking the live player/view camera

Before the latest mirror-camera math change, the reflected view also appeared to sit too far behind the mirror plane, roughly mirroring the viewer offset with an exaggerated depth error.

After that change, the depth/placement issue changed shape, but the reflection is still upside down and the mirror view now looks pinned rather than head-relative.

## Repro

Current repro scene:

- [examples/vtuber-mirror-example.mms](../../examples/vtuber-mirror-example.mms)

Observed behavior:

- moving the player/view camera does not make the reflected camera track correctly
- the reflected scene orientation is vertically inverted
- mirror facing direction is closer to correct than before, but the final reflected pose is still wrong

## Expected behavior

For a planar mirror:

- the reflected camera position should be the active viewer camera reflected across the mirror plane
- the reflected camera orientation should match the physically expected mirrored basis
- the sampled image on the mirror surface should not appear upside down
- the reflected view should update continuously as the player/view camera moves

## Why this matters

This blocks the mirror feature from being usable even though the render-to-texture path and mirror material plumbing now work.

The remaining issues are no longer about pass wiring; they are about the correctness of the reflected camera transform itself.

## Current suspicion

The likely problem is in the camera-space/world-space conversion and reflection basis reconstruction inside the mirror system.

The recent change switched mirror derivation to use `CameraData.transform.matrix_world` instead of reconstructing pose from `eye_data.view`, which appears to have changed the failure mode from "wrong offset" to "fixed camera".

That suggests one or more of these are still wrong:

- which basis vector should be treated as forward vs back in engine camera convention
- whether the reflected basis needs an additional handedness correction
- whether `CameraData.transform.matrix_world` is valid/current for the active window camera in the non-XR path
- whether the mirror pass needs a vertical flip in view/projection/viewport space instead of in the reflected camera basis

## Investigation targets

- [src/engine/ecs/system/mirror_system.rs](../../src/engine/ecs/system/mirror_system.rs)
- [src/engine/ecs/system/camera_system.rs](../../src/engine/ecs/system/camera_system.rs)
- [src/engine/ecs/system/openxr_system.rs](../../src/engine/ecs/system/openxr_system.rs)
- [src/engine/graphics/vulkano_renderer.rs](../../src/engine/graphics/vulkano_renderer.rs)
- [src/engine/graphics/visual_world.rs](../../src/engine/graphics/visual_world.rs)

Questions to answer:

- for the window camera path, is `CameraData.transform.matrix_world` actually populated with the active camera world transform every frame?
- does the engine treat camera local forward as `-Z` while the cached world matrix column `2` represents local `+Z` / back?
- is the upside-down result coming from the reflected basis itself, or from the renderer's viewport/projection convention for offscreen mirror passes?
- do mirrors need an explicit handedness fix after reflecting the camera frame across a plane?

## Notes

The mirror pass plumbing, runtime texture publication, mirror material routing, and self-exclusion work landed separately and should be treated as already solved unless new evidence says otherwise.

This bug note is specifically about the correctness of the reflected camera pose and orientation.
