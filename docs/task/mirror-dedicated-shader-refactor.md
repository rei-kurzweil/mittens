# Task: mirror dedicated shader refactor

## Status

Open follow-up.

## Current state

The mirror is now upright, which strongly suggests the vertical inversion was a sampling-orientation
issue rather than only a reflected-camera basis issue.

However, the reflection still appears horizontally flipped relative to the expected mirror result.

At the same time, the current implementation handles mirror-only texture orientation through
mirror-specific logic inside the shared toon/emissive textured shader path. That was acceptable as
a narrow diagnostic step, but it is not the right long-term design.

## Why this needs a refactor

`MaterialHandle::MIRROR` is a distinct material concept:

- it samples a runtime-produced reflection texture
- it may need mirror-specific UV/orientation handling
- it may later need Fresnel/tint/roughness/blur or portal-like variants

That behavior should live in a dedicated mirror shader and pipeline, not as ad hoc flags inside the
generic textured toon shader path.

## Goal

Refactor mirror surface rendering so `MaterialHandle::MIRROR` uses its own shader/pipeline, while
also fixing the remaining horizontal orientation error in a mirror-local way.

## Current code shape

- [src/engine/graphics/vulkano_renderer.rs](/home/rei/_/cat-engine/src/engine/graphics/vulkano_renderer.rs)
  currently treats `MaterialHandle::MIRROR` as part of the generic textured material path
- [assets/shaders/toon-mesh.frag](/home/rei/_/cat-engine/assets/shaders/toon-mesh.frag)
  and [assets/shaders/emissive-toon-mesh.frag](/home/rei/_/cat-engine/assets/shaders/emissive-toon-mesh.frag)
  now contain temporary mirror-oriented UV handling through shared material state
- [src/engine/graphics/primitives.rs](/home/rei/_/cat-engine/src/engine/graphics/primitives.rs)
  defines `MaterialHandle::MIRROR`, but there is still no dedicated `Material::MIRROR`

## Required refactor

1. Add a dedicated mirror fragment shader, e.g. `assets/shaders/mirror-mesh.frag`.
2. Add an explicit `Material::MIRROR` definition in [src/engine/graphics/primitives.rs](/home/rei/_/cat-engine/src/engine/graphics/primitives.rs).
3. Compile and wire a dedicated mirror graphics pipeline in [src/engine/graphics/vulkano_renderer.rs](/home/rei/_/cat-engine/src/engine/graphics/vulkano_renderer.rs).
4. Route `MaterialHandle::MIRROR` draw batches to that pipeline instead of the generic toon/emissive path.
5. Move mirror-specific UV orientation handling out of the shared toon/emissive shaders and into the mirror shader only.
6. Remove the temporary shared-material UBO fields or flags added only for mirror sampling if nothing else still needs them.

## Mirror shader expectations

The dedicated mirror shader should:

- sample the runtime mirror texture using the surface UVs
- own any mirror-specific UV flips or swizzles
- start simple and unlit
- preserve alpha/discard behavior compatible with the current textured path

For now, it does not need physically-based shading. The immediate job is correct sampling and
clean separation of concerns.

## Horizontal flip investigation

When doing this refactor, explicitly verify whether the remaining horizontal error is:

- the expected left-right reversal of a real mirror, or
- an extra unwanted horizontal flip introduced by surface UV orientation or shader sampling

That distinction matters:

- a real planar mirror should reverse left-right perceptually
- the mirror shader should not add an additional accidental horizontal inversion on top of that

Use [examples/vtuber-mirror-example.mms](/home/rei/_/cat-engine/examples/vtuber-mirror-example.mms:1)
as the primary repro scene.

## Done when

- `MaterialHandle::MIRROR` no longer depends on mirror-specific branches in the generic toon or
  emissive textured shaders
- mirror sampling behavior lives in a dedicated mirror shader/pipeline
- the mirror is no longer upside down
- the remaining horizontal behavior is understood and either fixed or documented as physically
  expected
