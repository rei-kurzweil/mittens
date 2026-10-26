# Opt-in broad bloom pyramid

Date: 2026-10-26
Status: follow-up request recorded; not implemented.

The Sakura rooftop scene needs an optional broader, softer sun glow. Retain the
current bloom path and defaults. Add an explicit opt-in mode that downsamples the
emission source repeatedly, filters at smaller resolutions, then upsamples and
combines contributions. More levels should allow a larger angular/screen-space
halo without requiring a huge full-resolution blur kernel.

Authoring decisions to review before implementation:

- A bounded level count controls how far the pyramid descends and therefore the
  broadest available glow. Define sensible behavior for very small targets.
- Keep overall intensity separate from spread/level weighting, so increasing
  depth does not unexpectedly multiply the total brightness.
- Specify whether radius remains in NDC, and how it interacts with level count.
- An optional broad-layer weight could control the soft outer halo separately
  from the compact bright core. Avoid requiring a different parameter per level
  unless a demonstrated authoring need warrants it.
- The existing mode remains the default, and unused pyramids allocate no images
  or extra GPU passes. Retire/reuse images on mode, extent and view-count changes.
- Both desktop and XR eyes need consistent scale and energy, with bounded image
  storage. Integrate with the ordered submission/resource lifetime path.

Preserve foreground occlusion of background sun emission. Test against tiled
building silhouettes and balconies: bloom should soften bright edges without
allowing an unoccluded sun source to shine through opaque buildings. Include
stereo, resize, disabling/re-enabling bloom, small viewports and no-emission cases.

The first Sakura example uses today's EmissivePass/Bloom settings. Its gradient
uses plain Background and emits no bloom; the sun uses a separate
Background.occlusion_and_lighting group with an emissive Toon material. This
existing route enables bloom extraction without making the sky depend on lights.
