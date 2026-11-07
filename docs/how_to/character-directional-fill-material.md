# Experimental directional fill for ship characters

The ship's avatar and face presets use a separate custom Anime fragment shader
to prevent an overhead light from producing the back-facing band under a face.
The standard Anime shader, truck preset and environment preset are unchanged.
This affects the local Miku vendor through its existing `ship_avatar_material()`
and `ship_face_material()` assignments, as well as the ship's other characters.
No restricted model or local example is published.

## Controls

In `assets/components/materials/ship_anime_materials.mms`, both character presets
set:

```mms
unshaded_axis = [0.0, 1.0, 0.0]
unshaded_min_elevation_degrees = 25.0
```

The axis is in **world space**. Elevation is measured above the plane perpendicular
to that axis. With world-up, it is the light's elevation above horizontal.
A zero axis disables this override. The angle is clamped to -90..90 degrees.

For each light, let `L` be the normalized direction **from the surface to the
light** (the opposite of the direction its rays travel):

- Ordinary facing surfaces (`N·L > 0`) accumulate the usual distance/cone-attenuated
  intensity, with the same binary facing gate as the existing Anime shader.
- A back-facing/tangent surface (`N·L <= 0`) with an affecting light inside the
  selected angular region marks a fill override. At 25 degrees, this means
  `dot(L, world_up) > sin(25°)`, approximately 0.4226.
- After summing lights, that override floors the ramp input at 1.0 once. It
  does not add a unit of illumination per light or reduce larger inputs. The
  current character lit threshold is 0.85, so an input of 1.0 is fully lit.
- A light with zero effective intensity, outside its range, or outside a spot
  cone cannot trigger the override. This is an artistic ramp rule, not a new
  shadow-map/occlusion system. Very weak but nonzero lights can still trigger it.
- The existing single view-dependent rim is computed independently afterward.

The cutoff is currently hard; no angular feathering has been introduced. Normal
lighting/shading still applies outside the angular region and where no qualifying
light affects the surface. Textures, alpha discard, shade/rim controls and the
rest of the ramp behavior match the standard Anime fragment shader.

`anime_directional_fill.mms` lazily creates one shared definition, so the body
and face parameter instances reuse its compiled shader and GPU programs.

## Required engine integration

Custom definitions previously had only a static-geometry program. They now have
distinct static and cached-deformation program handles; Vulkan pairs the same
custom fragment shader with the existing cached-skinned vertex shader and input
layout. Joint-matrix updates and deformation dispatch are unchanged. No new MMS
syntax or host calls were added.

The Vulkan custom-program preparation currently caches opaque/transparent
pipelines for both geometry variants (four per loaded custom definition, plus
extraction variants for an extracted-emissive base). This is startup/resource
cost, not pipeline creation per draw/frame. Truly unused definitions still have
no GPU programs. More selective geometry-variant preparation can be a later
startup optimization; shader definitions should be shared between instances.

Custom parameter snapshots now use the existing bounded LRU binding cache:
32 texture/filter bindings per material instance by default, up to eight
retained numeric revisions per binding. The former flat eight-slot pool mixed
texture bindings with revisions and failed on multi-texture avatars. Eviction
drops cache ownership only; recorded command buffers retain their resources.
The snapshot regression test covers retained revisions, ten thousand edits,
more than eight textures, bounded binding eviction and retained image views.

For validation and historical timing, see the
[benchmark ledger](../task/epic/vulkan-rendering-optimization/benchmark-history.md).
The first two directional-fill captures were invalid and are explicitly excluded
in the ignored local benchmark README.
