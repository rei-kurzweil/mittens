# Mirror in VR falls back to a static monoscopic perspective when a window camera is also active

## Status

Open bug note.

## Symptom

In VR, a `Mirror` can appear to render from a static monoscopic perspective instead of a proper
per-eye stereo reflection.

Observed behavior so far:

- when both XR and a monoscopic `Camera3D` are active, the mirror seen in VR can look like a
  single flat capture rather than a stereo reflection
- the same mirror still behaves as expected for the desktop / monoscopic active camera
- the issue appears specific to how mirror captures are selected and published for XR, not to the
  basic desktop mirror path

## Repro

General repro shape:

- run a scene with an active XR camera
- also keep an active monoscopic `Camera3D`
- view a surface using `Mirror`
- compare the mirror in desktop mode vs. inside the headset

Current expectation:

- desktop should use the monoscopic mirror capture
- XR should use the stereoscopic mirror capture matching the current eye

Current observed result:

- desktop uses the mirror correctly
- VR can show a static/shared perspective across both eyes

## Likely cause

There are two concrete code paths that look suspicious.

### 1. Mirror texture attachment is still hardcoded to `mono.0`

In [src/engine/ecs/system/mirror_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/mirror_system.rs:590),
the authored `TextureComponent` attached to the mirror surface is always pointed at:

- `capture.mirror.{guid}.mono.0.color`

That is correct for the desktop path, but it is not a view-family-aware default for XR.

The renderer does have a per-render-view retarget step in
[src/engine/graphics/vulkano_renderer.rs](/home/rei/_/cat-engine/src/engine/graphics/vulkano_renderer.rs:2291),
which tries to swap mirror surface textures to the matching family/view at draw time, but the
component-level attachment itself remains monoscopic.

That means the XR result is vulnerable to any path that relies on the registered texture selector
instead of the renderer's late retarget.

### 2. Mirror offscreen rendering indexes captures by slot, not by family/view

In [src/engine/graphics/vulkano_renderer.rs](/home/rei/_/cat-engine/src/engine/graphics/vulkano_renderer.rs:3788),
the mirror offscreen render loop does:

- `let view_count = mirror.captures.len();`
- `for eye in 0..view_count`
- `let capture = mirror.captures.get(eye)`

But `mirror.captures` is built in
[src/engine/ecs/system/mirror_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/mirror_system.rs:364)
by appending families in this order:

- window monoscopic capture(s) first
- XR stereoscopic capture(s) after that

So when both families exist, the capture vector is shaped like:

- `mono.0`
- `stereo.0`
- `stereo.1`

The renderer currently treats those as generic eye slots instead of selecting captures by their
actual `(family, view_index)` identity.

That does not match the logical XR eye set and is a plausible reason the mirror can appear to use
the wrong source perspective in VR when a monoscopic camera is also present.

## Why this is probably the right direction

The symptom only appears when a monoscopic camera is also active, which lines up with the capture
list becoming a mixed-family list instead of a pure stereo pair.

If XR were the only active viewer family, `mirror.captures` would contain only the stereo views,
so this family-mixing bug would be masked.

## Investigation targets

- [src/engine/ecs/system/mirror_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/mirror_system.rs)
- [src/engine/graphics/visual_world.rs](/home/rei/_/cat-engine/src/engine/graphics/visual_world.rs)
- [src/engine/graphics/vulkano_renderer.rs](/home/rei/_/cat-engine/src/engine/graphics/vulkano_renderer.rs)

## Suggested fix direction

- stop storing the mirror surface's authored runtime texture selector as a permanently hardcoded
  monoscopic key
- render mirror capture targets by explicit `(family, view_index)` selection instead of indexing
  directly into `mirror.captures`
- if needed, split mirror offscreen target allocation/rendering by viewer family so XR only renders
  the stereo captures it will actually consume for the headset view

## Notes

This note is about the viewer-family selection/publishing path.

It is separate from the existing reflected-pose correctness bug documented in
[mirror-camera-orientation-and-tracking.md](/home/rei/_/cat-engine/docs/bugs/mirror-camera-orientation-and-tracking.md).
