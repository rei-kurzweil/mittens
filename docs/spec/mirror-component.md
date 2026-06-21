# Mirror Component

This document describes the authored `MirrorComponent`: what it means, how it is attached, and
what behavior authors should expect from a mirror surface in `cat-engine`.

For internal runtime details, see
[mirror-system.md](/home/rei/_/cat-engine/docs/spec/mirror_system.md).

## Purpose

`MirrorComponent` marks a scene surface as a planar mirror.

At runtime, the engine:

- derives one or more reflected capture views from the active viewer family
- renders those views into offscreen targets
- publishes the resulting image as a runtime texture
- routes the mirror surface to mirror material sampling

The component itself is authored data only. It does not store camera matrices or render targets.

## Authored Shape

Current component shape in `src`:

```rust
pub struct MirrorComponent {
    pub quality: i32,
}
```

Current behavior:

- `quality` is clamped to `64..=2048`
- default quality is `512`
- MMS authoring uses `Mirror.quality(N)`

Source:

- [src/engine/ecs/component/mirror.rs](/home/rei/_/cat-engine/src/engine/ecs/component/mirror.rs)

## Attachment Rule

`MirrorComponent` should be attached under the visual object that represents the reflective surface.

In current runtime terms:

- `MirrorSystem` finds the nearest ancestor `TransformComponent`
- that transform defines the mirror plane and local basis
- `MirrorSystem` then finds the parent `RenderableComponent`
- that renderable becomes the mirror surface whose material/texture are overridden for sampling

Practical rule for authors:

- author the mirror as a renderable surface with a stable transform
- place `MirrorComponent` under that renderable subtree

## Plane and Surface Convention

The mirror plane is defined by the nearest ancestor transform:

- local `XY` is the reflective surface
- local `+Z` is the mirror normal

For thick mirror geometry, the runtime currently shifts the reflection plane onto the renderable's
visible local `+Z` face rather than reflecting from the volume center. That keeps the reflection
aligned to the visible front surface instead of the middle of the slab.

Implications for authored meshes:

- the surface should be modeled so its reflective face corresponds to local `+Z`
- rotating the mirror rotates the reflection plane
- non-uniform scale affects the authored world-space size of the mirror window

## Quality

`quality` is a resolution preference for the mirror capture.

What authors can expect:

- higher values increase mirror texture sharpness
- higher values also increase mirror render cost
- the renderer derives the final offscreen extent from this preference and the mirror aspect ratio

This is a quality knob, not a guarantee of an exact square target size in all cases.

## Viewer Behavior

A mirror reflects the active viewer family each frame:

- monoscopic window rendering uses the active window camera
- stereoscopic rendering uses the active XR views

The reflected image should behave as a planar reflection:

- moving relative to the mirror updates the reflection continuously
- geometry touching the mirror edge in world space should continue seamlessly into the reflection
- off-center viewing should remain connected at the mirror edges

The internal projection math that enforces that behavior is documented in
[mirror-system.md](/home/rei/_/cat-engine/docs/spec/mirror_system.md).

## Material / Texture Contract

Current runtime behavior:

- `MirrorSystem` forces the parent renderable to use `MaterialHandle::MIRROR`
- `MirrorSystem` ensures a `TextureComponent.render_image(...)` points at the mirror capture key

That means authors do not currently bind the mirror capture texture manually in normal usage. The
mirror runtime handles that binding contract.

## Example

Example MMS shape:

```mms
T.position(0.0, 0.55, -4.5).scale(3.0, 3.0, 0.08) {
    R.cube() {
        C.rgba(0.82, 0.88, 0.94, 1.0)
        Mirror.quality(1024) {}
    }
}
```

Related example files:

- [examples/vtuber-mirror-example.mms](/home/rei/_/cat-engine/examples/vtuber-mirror-example.mms)
- [examples/vtuber-mirror-example.rs](/home/rei/_/cat-engine/examples/vtuber-mirror-example.rs)

## Non-Goals

`MirrorComponent` does not currently expose:

- custom clip-plane settings
- recursion depth settings
- per-mirror material variants
- explicit source camera selection

Those are runtime policy concerns, not authored component fields in the current implementation.
