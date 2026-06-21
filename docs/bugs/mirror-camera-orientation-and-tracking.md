# Mirror camera orientation and tracking are wrong

## Status

Open bug / regression note.

The viewer-family capture refactor has landed far enough that mirrors can now publish separate
captures for active monoscopic and active stereoscopic camera families. That was necessary, but it
did not resolve the remaining reflected-pose bug.

## Symptom

The current mirror rendering path still has visible reflected-camera bugs:

- the reflected view under-tracks the live viewer pose
- the visible reflection has a gap/parallax error against geometry that should meet the mirror edge
- the gap grows as the viewer moves farther from the mirror
- when the avatar is brought very close to the mirror, the reflected face/head motion still does
  not follow as far as the live movement should

Before the latest mirror-camera math change, the reflected view also appeared to sit too far behind the mirror plane, roughly mirroring the viewer offset with an exaggerated depth error.

After that change, the depth/placement issue changed shape, but the mirror view still does not
behave as a correct head-relative planar reflection.

## Repro

Current repro scene:

- [examples/vtuber-mirror-example.mms](../../examples/vtuber-mirror-example.mms)

Observed behavior:

- moving the player/view camera does not make the reflected camera track correctly
- side planes or markers placed flush against the mirror edge do not continue seamlessly into the
  reflection
- the seam grows with viewer distance, which strongly suggests the reflected camera or clip plane
  is not matching the visible mirror surface
- moving the viewer upward/downward close to the mirror does not make the reflected face/head move
  as far as it should
- mirror facing direction is closer to correct than before, but the final reflected pose is still
  wrong
- making the mirror square in world space does not remove the bug, so the current failure is not
  explained by mirror aspect ratio alone
- when the viewer stands near the left or right edge of the mirror and looks directly at it, the
  reflected face/head still does not appear where a correct planar reflection should show it
- when the viewer gets very close and then looks at the mirror from an angle, the reflected
  grid/world alignment becomes dramatically wrong rather than slightly offset
- the world grid provides a strong vertical-alignment repro: if the live camera is vertically
  aligned so the grid plane cuts through the middle of the live view, the reflected grid should
  appear at the matching reflected height, but currently appears elevated within the mirror image
- when the viewer places their eyes inside the grid plane and faces the mirror, the reflected grid
  also appears as a line/cross-section, but at the wrong height inside the mirror image

## Expected behavior

For a planar mirror:

- geometry that physically touches the mirror edge should appear to continue seamlessly into the
  reflected scene
- the reflected camera position should be the active viewer camera reflected across the mirror plane
- the reflected camera orientation should match the physically expected mirrored basis
- the reflected view should update continuously as the player/view camera moves
- bringing the avatar close to the mirror should make the reflected face/head motion track the live
  motion at the same magnitude

## Why this matters

This blocks the mirror feature from being usable even though the render-to-texture path and mirror material plumbing now work.

The remaining issues are no longer about pass wiring; they are about the correctness of the reflected camera transform itself.

## Current suspicion

The likely problem is now in one or more of:

- the exact mirror plane origin used for the reflection and oblique clip plane
- the camera-space/world-space conversion for the active viewer pose
- the reflected basis reconstruction inside the mirror system
- the oblique near-plane projection used to clip to the mirror surface

The viewer-family capture split was the right structural change, but the remaining symptom is still
present even after that work. That means the current blocker is not "wrong source family" alone.

That suggests one or more of these are still wrong:

- whether the mirror plane is aligned to the actually visible reflective surface for the authored
  renderable
- which basis vector should be treated as forward vs back in engine camera convention
- whether the reflected basis needs an additional handedness correction
- whether `CameraData.transform.matrix_world` is valid/current for the active window camera in the non-XR path
- whether the oblique clip plane is derived from the same plane the reflection math uses
- whether the mirror pass needs a vertical flip in view/projection/viewport space instead of in the
  reflected camera basis
- whether the reflected camera is preserving the wrong in-plane coordinates when expressed in the
  mirror surface basis
- whether the reflected camera's vertical placement relative to the mirror plane is wrong even when
  the live camera is aligned to a world-space reference plane like the grid

## Investigation targets

- [src/engine/ecs/system/mirror_system.rs](../../src/engine/ecs/system/mirror_system.rs)
- [src/engine/ecs/system/camera_system.rs](../../src/engine/ecs/system/camera_system.rs)
- [src/engine/ecs/system/openxr_system.rs](../../src/engine/ecs/system/openxr_system.rs)
- [src/engine/graphics/vulkano_renderer.rs](../../src/engine/graphics/vulkano_renderer.rs)
- [src/engine/graphics/visual_world.rs](../../src/engine/graphics/visual_world.rs)

Questions to answer:

- is the mirror plane origin exactly the visible reflective surface, not the renderable center or
  frame depth center?
- for the window camera path, is `CameraData.transform.matrix_world` actually populated with the active camera world transform every frame?
- does the engine treat camera local forward as `-Z` while the cached world matrix column `2` represents local `+Z` / back?
- does the reflected view under-track because the camera position is reflected across the wrong
  plane, or because the reflected basis/projection is wrong after that point?
- if the viewer position is decomposed in mirror-local coordinates, are the in-plane coordinates
  being preserved and only the plane-normal coordinate negated, exactly as planar reflection
  requires?
- why does a viewer standing at the mirror's horizontal edge while facing it not see the expected
  reflected face/head within the mirror bounds?
- why does the reflected grid appear elevated inside the mirror image when the live camera is
  vertically aligned with the same world-space grid plane?
- why does looking at the mirror from an oblique angle make the reflected grid/world alignment fail
  dramatically rather than preserving a consistent planar reflection?
- does the oblique clip projection use the same plane origin/normal as the reflection pose itself?
- is the upside-down result coming from the reflected basis itself, or from the renderer's
  viewport/projection convention for offscreen mirror passes?
- do mirrors need an explicit handedness fix after reflecting the camera frame across a plane?

## Notes

The mirror pass plumbing, runtime texture publication, mirror material routing, self-exclusion, and
viewer-family capture split should be treated as largely solved unless new evidence says otherwise.

This bug note is specifically about the correctness of the reflected camera pose and orientation.
