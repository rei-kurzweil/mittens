# Grid Surface material and native Grid migration

Status: a standard Surface material asset is implemented; native editor Grid
runtime migration remains a separate integration slice.

## Implemented asset

`assets/components/materials/grid_surface.mms` shares one
`MaterialDefinition(MaterialBase())` using `assets/shaders/grid-surface.frag`.
The shader consumes the existing Surface vertex interface and outputs flat grid
color/coverage, without the legacy Grid pipeline, Toon lighting, emissive
extraction or a quantization parameter.

Named inputs: `spacing`, `world_space`, `local_extent`, `minor_width`,
`major_width`, `axis_width`, `fade_start` and `fade_end`. Color/opacity come from
the ordinary renderable inputs. `world_space` selects dominant world-plane axes;
local mode uses centered UV coordinates scaled by `local_extent`. Match that
extent to the displayed plane's desired local grid dimensions.

`examples/grid-material.mms` displays independent local and world grid materials
that share the definition. Run with `cargo run --release -- load examples/grid-material.mms`.
They can be applied to ordinary renderable planes without a special material
base or new renderer pipeline. Spacing edits update retained material inputs.

## Native editor Grid integration still to do

The current native Grid runtime creates a thin cube using GRID_MESH and a
LightQuantizationComponent carrying signed spacing. Its dedicated vertex shader
computes plane-local coordinates by projecting world position relative to model
origin onto normalized model X/Z axes. World mode selects dominant plane axes.
Do not silently replace that with unscaled UVs when migrating it.

- [ ] Define native-to-material mapping for spacing, world/local selection,
  displayed extent and arbitrary owner transforms, preserving existing semantics.
- [ ] Reuse the Surface definition for native live/preview grids and update named
  inputs when spacing, transform, extent or visual-space settings change.
- [ ] Keep painting, snapping, selection, serialization and preview opacity
  independent from material appearance. Native Grid's behavior remains native.
- [ ] Verify translated, rotated, nonuniformly scaled and nested grids, including
  coordinate-space toggles, animated owner transforms and both XR eyes.
- [ ] Remove the native runtime's LightQuantization transport and dedicated Grid
  pipeline only after its active consumers use the standard definition.

During this transition, ordinary legacy Toon shading is fixed at three bands.
The old uniform slot remains available specifically for native Grid spacing;
its typed binding key distinguishes spacing rather than lighting quantization.
This is compatibility for the existing editor runtime, not a fallback for the
new Surface asset. No Android rendering feature or legacy pipeline is added.

Related: [rendering optimization epic](epic/vulkan-rendering-optimization/README.md).
