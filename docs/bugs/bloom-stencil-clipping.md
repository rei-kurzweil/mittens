# Bloom should respect stencil clipping

## Status

Open bug / investigation note.

## Symptom

Bright emissive content inside a stencil-clipped viewport is geometrically clipped in the main scene, but its bloom/glow is not clipped to the same boundary.

The leaked glow extends outside the clip region even when the underlying emissive source pixels are correctly masked.

## Repro

Current repro scene:

- [examples/ui-layout.mms](../../examples/ui-layout.mms)

Observed behavior in that scene:

- the MMS-authored stencil-clipped panel clips emissive objects themselves
- the resulting bloom halo still bleeds outside the panel bounds
- this happens even when the world panel is left on the opaque, non-overlay path

## Expected behavior

Stencil clipping should constrain both:

- the source renderables seen in the main/emissive passes
- the final visible bloom contribution produced from those renderables

If a pixel is outside the clip mask, neither the source emissive fragment nor the bloom derived from it should appear there.

## Why this matters

For scroll views, panels, and authored MMS viewports, unclipped bloom breaks the visual contract of the viewport.

It makes clipped UI look correct at the geometry level but wrong at presentation time, especially for text and other small bright elements.

## Likely cause

The clip mask currently appears to affect scene/emissive rendering, but not the later bloom composite in the same way.

Likely failure modes:

- bloom extraction is correctly based on emissive/color content, but the blur/composite stage no longer has clip-mask awareness
- stencil state is not preserved or re-applied when compositing bloom back onto the final target
- the renderer treats bloom as a fullscreen post-process over resolved textures, after clip information has already been discarded

## Investigation targets

- [src/engine/graphics/post_processing.rs](../../src/engine/graphics/post_processing.rs)
- [src/engine/graphics/vulkano_renderer.rs](../../src/engine/graphics/vulkano_renderer.rs)
- [src/engine/graphics/visual_world.rs](../../src/engine/graphics/visual_world.rs)
- [src/engine/ecs/system/system_world.rs](../../src/engine/ecs/system/system_world.rs)

Questions to answer:

- where does stencil clipping stop influencing the pipeline?
- does the emissive extraction target contain already-clipped pixels only, or can bright values survive in a way that still blooms past the boundary?
- should bloom be clipped during extraction, during blur, during composite, or via a separate published clip mask?

## Likely fix directions

Possible approaches, depending on the intended renderer architecture:

1. ensure emissive extraction writes only clipped pixels, with no later stage reintroducing contribution outside the clip
2. carry a clip-aware mask or alpha through the bloom pipeline and apply it during composite
3. re-apply clip constraints when compositing bloom into the final scene, if clip semantics are meant to survive post-processing

The preferred fix should match the engine's long-term render-graph model rather than adding a one-off special case for MMS panels.
