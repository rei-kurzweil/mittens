# Task: renderer emission capabilities and custom LED strip material

Date: 2026-10-09
Status: proposed; no implementation in this documentation pass.
Parent: [custom materials epic](epic/custom-materials.md).
Release: [0.10.0](epic/0.10.0/README.md).
Depends on: [Material contract](material-component-and-resolved-contract.md)
and the adapted [custom-fragment proof](mms-custom-fragment-shader-first-slice.md).

Scene/asset preparation: [four-edge custom-materials example](custom-materials-led-strip-example-preparation.md).
Use `examples/custom-materials.mms` for the proof: custom `animated_led_strip`
on combined LED rectangles only; separate regular Toon plastic/backing.

## Goal

A custom fragment material can emit an animated LED pattern and contribute
that same pattern to the existing emissive extraction/Bloom path. Participation
uses declared renderer capabilities instead of emissive Toon handle identity.

## Work and gates

- [ ] Inventory emission handle conversion in RenderableSystem, classification
  in VisualWorld, pipeline selection in draw recording, extraction shaders,
  depth/coverage, background eligibility, and post-process activation/order.
- [ ] Define emission radiance/intensity, main-color behavior, extraction
  capability, and renderer Bloom configuration as separate fields/owners.
- [ ] Choose shared/generated surface-and-emission pass variants or an explicit
  emission fragment contract. Specify compatible outputs/descriptors and reuse
  the same retained input revision, UV pattern, alpha coverage, and deformation
  in all supported passes. A fixed Toon extraction shader cannot describe an
  arbitrary custom LED pattern.
- [ ] Resolve pass/batch/pipeline participation by capability, with existing
  handles mapped through a compatibility bridge. Reuse shader modules/pipelines
  where compatible; keep material values out of pipeline cache keys.
- [ ] Build one opaque static-mesh LED strip example with a finite `f32` glow
  input updated through explicit MMS setters, then verify supported GLTF/
  cached-deformed variants. Clock/beat sourcing is explicit, not a draw callback.
- [ ] Validate clipping/cutout/depth/background combinations inherited from
  existing emission behavior; reject custom variants not yet supported.
- [ ] Measure resources, uploads, draw/pass counts, and frame cost in desktop
  and XR, with Bloom disabled/enabled and shared/independent material instances.

Gate: dark LED segments remain dark in the extraction source while lit segments
match the main-color pattern; foreground depth and supported coverage suppress
emission correctly; no double application changes intensity. Updating many
distinct glow values neither recompiles shaders nor grows persistent UBO/
descriptor allocations after warm-up. Removal/reconfiguration retires resources
safely for frames in flight. Existing emissive Toon scenes remain valid, and
disabled/unused capabilities add no extraction pass.

Broader transparent/transmissive custom materials and the known refraction/Bloom
halo-ownership policy require their own validation; this task does not claim to
solve them by renaming emission flags. Document the first supported phase matrix
and fallback behavior in the material/resource graph.
