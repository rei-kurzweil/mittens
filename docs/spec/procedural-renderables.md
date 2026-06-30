# Procedural Renderables

Date: 2026-06-30

Related how-to:

- [docs/how_to/procedural_renderables_in_mms.md](../how_to/procedural_renderables_in_mms.md)

## Goal

Document the engine-facing procedural `Renderable` surface:

- which built-in procedural renderables exist
- what their Rust/API constructors are
- what parameters they take
- how they are expected to round-trip through MMS authoring

This doc is API/spec oriented. For authored `.mms` examples, see:

- [docs/how_to/procedural_renderables_in_mms.md](../how_to/procedural_renderables_in_mms.md)

## Current procedural renderables

These renderables are generated procedurally by engine code and registered into
`RenderAssets` at spawn time.

Currently implemented:

- `partial_annulus_2d`
- `star`
- `heart`

## Rust component constructors

The current Rust-facing constructors live on:

- [src/engine/ecs/component/renderable.rs](../../src/engine/ecs/component/renderable.rs)

### `RenderableComponent::partial_annulus_2d(...)`

```rust
RenderableComponent::partial_annulus_2d(
    render_assets,
    inner_radius,
    outer_radius,
    start_angle_radians,
    sweep_angle_radians,
    segments,
)
```

Parameter meanings:

- `inner_radius: f32`
- `outer_radius: f32`
- `start_angle_radians: f32`
- `sweep_angle_radians: f32`
- `segments: u32`

Geometry contract:

- 2D annulus arc in the XY plane
- front face normal is `+Z`
- if sweep is effectively `2π`, it falls back to the full annulus generator
- negative sweep is normalized to an equivalent positive arc

Implementation:

- [MeshFactory::partial_annulus_2d](../../src/engine/graphics/mesh.rs:598)

### `RenderableComponent::star(...)`

```rust
RenderableComponent::star(
    render_assets,
    points,
    inner_radius_fraction,
    outer_bevel_segments,
    inner_bevel_segments,
)
```

Parameter meanings:

- `points: u32`
- `inner_radius_fraction: f32`
- `outer_bevel_segments: u32`
- `inner_bevel_segments: u32`

Geometry contract:

- 2D filled star in the XY plane
- front face normal is `+Z`
- `points` is clamped to at least `3`
- `inner_radius_fraction` is clamped into a safe range
- `0` bevel segments means a sharp corner at that corner family

Implementation:

- [MeshFactory::star](../../src/engine/graphics/mesh.rs:667)

### `RenderableComponent::heart(...)`

```rust
RenderableComponent::heart(render_assets, segments)
```

Parameter meanings:

- `segments: u32`

Geometry contract:

- 2D filled heart in the XY plane
- front face normal is `+Z`
- silhouette is sampled from a standard heart curve
- `segments` is clamped to a practical minimum for stability

Implementation:

- [MeshFactory::heart](../../src/engine/graphics/mesh.rs:710)

## MMS-facing constructor names

These Rust constructors are exposed to MMS as:

- `R.partial_annulus_2d(...)`
- `R.star(...)`
- `R.heart(...)`

The MMS materialization path lives in:

- [src/meow_meow/component_registry.rs](../../src/meow_meow/component_registry.rs:1072)

## Round-trip / authored provenance

Procedural renderables store authored shape metadata on:

- `AuthoredRenderableShape`

in:

- [src/engine/ecs/component/renderable.rs](../../src/engine/ecs/component/renderable.rs:10)

That metadata is used so saving / re-emitting MMS can preserve:

- constructor name
- constructor arguments

Current emitted MMS forms:

- `Renderable.partial_annulus_2d(...)`
- `Renderable.star(...)`
- `Renderable.heart(...)`

## Example files

MMS example:

- [examples/pride.mms](../../examples/pride.mms)

Runtime wrapper:

- [examples/pride.rs](../../examples/pride.rs)

For a usage-oriented walkthrough with labeled parameter examples, see:

- [docs/how_to/procedural_renderables_in_mms.md](../how_to/procedural_renderables_in_mms.md)
