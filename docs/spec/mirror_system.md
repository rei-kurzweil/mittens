# Mirror System

This document describes the internal runtime implementation of planar mirrors in `cat-engine`.

It covers:

- mirror discovery and registration
- mirror plane and surface derivation
- reflected camera/view construction
- off-axis projection construction
- runtime texture publication
- renderer-facing mirror capture shape

For authored usage, see
[mirror-component.md](/home/rei/_/cat-engine/docs/spec/mirror-component.md).

## Scope

The current runtime implementation is centered on:

- `MirrorComponent`
- `MirrorSystem`
- `VisualMirror`
- mirror `RenderView`s in the renderer

Primary code:

- [src/engine/ecs/system/mirror_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/mirror_system.rs)
- [src/engine/graphics/visual_world.rs](/home/rei/_/cat-engine/src/engine/graphics/visual_world.rs)
- [src/engine/graphics/vulkano_renderer.rs](/home/rei/_/cat-engine/src/engine/graphics/vulkano_renderer.rs)

## Runtime Responsibilities

`MirrorSystem` runs every frame and is responsible for:

1. finding all `MirrorComponent`s
2. resolving the mirror transform and renderable surface
3. deriving world-space mirror dimensions and plane data
4. deriving reflected capture views for active viewer families
5. registering `VisualMirror` records on `VisualWorld`
6. wiring the mirror surface to `MaterialHandle::MIRROR`
7. ensuring the renderable subtree has a `TextureComponent.render_image(...)` for the mirror key

The renderer is then responsible for:

1. allocating mirror offscreen targets
2. rendering mirror `RenderView`s before the main pass
3. publishing mirror color results into runtime texture handles
4. sampling those runtime textures on the visible mirror surface

## Discovery Model

For each mirror component:

- the nearest ancestor `TransformComponent` is treated as the mirror transform
- the parent `RenderableComponent` is treated as the visible reflective surface
- bounds are read from the renderable or one of its children to determine mirror size/aspect

That means the authored ECS structure is important. The mirror plane comes from transform data, but
the visible surface dimensions come from renderable bounds.

## Mirror Plane Definition

The runtime mirror plane is derived from the resolved transform:

- plane origin starts at the transform world translation
- plane normal is the normalized world-space local `+Z` axis
- mirror-local `X` and `Y` come from the world transform basis

For thick renderables, the plane origin is then moved onto the visible local `+Z` face using the
resolved bounds. This keeps the reflection plane aligned to the visible front surface rather than
the center of the mesh volume.

In effect:

- plane basis comes from transform
- plane depth offset comes from renderable bounds

## Viewer Families

The runtime supports separate capture families:

- monoscopic captures from `CameraTarget::Window`
- stereoscopic captures from `CameraTarget::Xr`

For each active source family:

- each source view becomes one mirror capture request
- each capture request gets its own reflected `view` / `proj`
- the capture key is tagged by mirror GUID, family, and view index

This avoids the earlier incorrect model of one shared mirror texture for all viewers.

## Reflected Camera Construction

For each active source view:

1. read the live source eye world transform from `VisualWorld`
2. read source eye world position
3. reflect that point across the mirror plane to get reflected eye position
4. build a mirror-aligned camera basis using:
   - forward = mirror plane normal
   - up = mirror local `+Y`
5. build a reflected world matrix from that basis and reflected eye position
6. invert it to get the reflected view matrix

Important current rule:

- the reflected eye position follows the live viewer
- the capture basis stays aligned to the mirror plane
- viewer-relative offset is encoded by the off-axis frustum, not by rotating the capture basis with
  viewer yaw/pitch

That rule is what keeps edge-touching reflected geometry connected when the viewer looks up, down,
left, or right relative to the mirror.

## Projection Rule

The current implementation uses an off-axis projection derived from the reflected eye position and
the authored mirror rectangle.

It does not use:

- a source-camera FOV copy
- a symmetric FOV reconstructed only from mirror height and eye-to-plane distance

Current projection algorithm:

1. compute the four mirror corners in world space from:
   - plane origin
   - mirror local `X`
   - mirror local `Y`
   - mirror world width / height
2. transform those corners into reflected camera space
3. project those corners onto the chosen near plane
4. derive `left`, `right`, `bottom`, and `top`
5. build a right-handed zero-to-one off-axis perspective matrix
6. preserve source near/far policy

Why this matters:

- translation relative to the mirror changes the frustum center
- off-center viewing must still keep reflected geometry attached to the mirror edges
- a symmetric frustum only works near the centered head-on case

## Clip Plane

There is support code for oblique near-plane clipping in `MirrorSystem`, but it is currently gated
behind a disabled constant.

Current state:

- helper math exists
- the feature is not enabled by default

So the current mirror implementation should be understood as:

- reflected camera pose: active
- off-axis projection: active
- oblique clip plane: present but disabled

## VisualWorld Contract

`MirrorSystem` registers mirrors on `VisualWorld` as `VisualMirror` records.

Current `VisualMirror` data includes:

- mirror component id
- capture requests
- plane origin
- plane normal
- aspect ratio
- source instance
- resolution scale

Each capture request includes:

- viewer family
- view index
- `CameraData { view, proj, transform }`
- runtime texture target key

This makes mirrors renderer-facing frame data rather than persistent ECS camera components.

## Material / Texture Wiring

After registering the mirror captures, `MirrorSystem` updates the resolved mirror renderable:

- material is forced to `MaterialHandle::MIRROR`
- a `TextureComponent` is created or updated
- that texture component points to the mirror runtime texture key

Current publication shape:

- `capture.mirror.<guid>.mono.0.color`
- `capture.mirror.<guid>.stereo.<view_index>.color`

The visible surface samples the monoscopic key by default through the existing texture routing
contract.

## Renderer Contract

The renderer consumes `VisualMirror` records and turns them into mirror `RenderView`s.

At a high level it must:

1. allocate or reuse mirror offscreen targets
2. render mirror views before the main pass that samples them
3. publish the mirror color output to the runtime texture bridge

This is intentionally parallel to the existing offscreen XR rendering path:

- same scene
- different view/proj
- different target
- same renderer-phase machinery

## Important Invariants

The current implementation depends on these invariants:

- the mirror plane basis comes from the resolved transform
- the source viewer transforms in `VisualWorld` are live each frame
- the reflected eye position is the exact plane reflection of the source eye position
- the mirror capture basis stays aligned to the mirror plane
- the projection is off-axis from the reflected eye to the mirror rectangle
- renderer Y-flip policy stays in the renderer, not in mirror camera basis math

When one of these fails, the typical symptoms are:

- reflection pinned in place
- geometry disconnecting from mirror edges
- shrinking toward the side the viewer turns toward
- reflection appearing upside down

## Related Docs

- [mirror-component.md](/home/rei/_/cat-engine/docs/spec/mirror-component.md)
- [mirror-component-camera.md](/home/rei/_/cat-engine/docs/spec/mirror-component-camera.md)
- [docs/task/mirror-camera-projection-debug-followup.md](/home/rei/_/cat-engine/docs/task/mirror-camera-projection-debug-followup.md)
- [docs/task/mirror-render-pass-status.md](/home/rei/_/cat-engine/docs/task/mirror-render-pass-status.md)
- [docs/bugs/mirror-camera-orientation-and-tracking.md](/home/rei/_/cat-engine/docs/bugs/mirror-camera-orientation-and-tracking.md)
