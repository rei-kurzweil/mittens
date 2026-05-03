# Task: Partial Annulus Primitive

Add support for spawning angular segments of an annulus (e.g. quarters, halves) as 2D primitives.

## Research Findings
- `MeshFactory::circle_2d(inner, outer, number_of_segments)` in `src/engine/graphics/mesh.rs` already generates a full 2D ring using `std::f32::consts::TAU`.
- `RenderAssets` supports dynamic registration of `CpuMesh` objects at runtime, but today it does not provide a general procedural-mesh cache keyed by mesh parameters. `register_mesh` always appends a new mesh.
- The only existing mesh-variant cache is the UV override cache in `RenderableSystem`, keyed by `base_mesh` plus UV bits.
- `RenderableComponent` can be constructed from any `CpuMeshHandle`, and derived meshes are expected to preserve the source identity in `renderable.base_mesh`.

## Proposed Changes

### 1. `src/engine/graphics/mesh.rs`
Add `MeshFactory::partial_annulus_2d`.

The API should not take `number_of_segments` directly. Instead, expose either:
- `full_annulus_segments: u32`

This is the preferred shape. The partial annulus then derives its actual emitted segment count from the angular span:

```rust
pub fn partial_annulus_2d(
    inner_radius: f32,
    outer_radius: f32,
    start_angle: f32,
    end_angle: f32,
    full_annulus_segments: u32,
) -> CpuMesh
```

Implementation requirements:
- Clamp `full_annulus_segments` to a sensible minimum.
- Compute `span = normalized end_angle - start_angle` in radians.
- Derive the emitted segment count from the fraction of a full turn:
  `partial_segments = ceil(full_annulus_segments * span / TAU)`.
- This means a partial annulus usually emits fewer segments than `full_annulus_segments`, because that input describes the tessellation density of the corresponding full ring.
- Generate outer and inner vertices as in `circle_2d`, but only over `[start_angle, end_angle]`.
- Do not wrap the final indices back to the first segment unless the span is effectively a full annulus.

`segment_delta_angle: f32` is a possible alternative API, but `full_annulus_segments` is preferred because:
- it stays consistent with the current `circle_2d(..., number_of_segments)` mental model
- it makes cache keys simpler and more stable
- it defines quality in terms of the equivalent full-ring tessellation, which is easier to reason about when authoring partial arcs

### 2. Base-mesh identity for partial annulus variants
Partial annulus meshes should be treated as a distinct base mesh kind, not as `CIRCLE_2D`.

The spec should preserve the same pattern already used elsewhere:
- `renderable.mesh` may point at a dynamically registered concrete mesh
- `renderable.base_mesh` should identify the underlying mesh family / source shape

For partial annulus that means:
- add a distinct base-mesh identity for `partial_annulus_2d`
- a concrete partial annulus instance should report `base_mesh = partial_annulus_2d`, not `circle_2d`

This matters because caches and downstream systems should distinguish:
- full annulus / ring meshes
- partial annulus meshes

even when both are dynamically registered CPU meshes.

Implementation note:
- today `base_mesh` is represented as a `CpuMeshHandle`, with stable built-in identities such as `CIRCLE_2D`
- so giving partial annulus its own `base_mesh` likely requires adding a new stable mesh kind / built-in identity for `partial_annulus_2d`, rather than only storing the concrete dynamic mesh handle

### 3. `RenderAssets` procedural mesh caching
Add a procedural CPU mesh cache in `RenderAssets` for parameterized generated meshes.

This is needed because today:
- `RenderAssets::register_mesh` always creates a fresh `CpuMeshHandle`
- there is no general deduplication for procedural meshes with the same parameters

The cache key for partial annulus variants should include:
- the base mesh kind: `partial_annulus_2d`
- `inner_radius`
- `outer_radius`
- `start_angle`
- `end_angle`
- `full_annulus_segments`

`full_annulus_segments` must participate in the key, because it changes tessellation and therefore changes the generated mesh.

The cache should return the same `CpuMeshHandle` for repeated requests with identical parameters.

### 4. Relationship to existing base-mesh-based caches
The existing UV mesh cache in `RenderableSystem` already keys derived meshes by `base_mesh` plus override data. That is a useful precedent, but it is not sufficient for procedural annulus generation by itself.

For partial annulus we need both:
- a base-mesh identity that says “this mesh came from partial_annulus_2d”
- a parameter cache in `RenderAssets` so equal partial-annulus requests reuse the same CPU mesh handle

In other words:
- `base_mesh` distinguishes the mesh family
- the procedural cache key distinguishes concrete variants within that family

### Usage Example
```rust
let mesh = universe.render_assets.get_or_register_partial_annulus_2d(
    0.4,
    0.5,
    0.0,
    std::f32::consts::FRAC_PI_2,
    64,
);
let renderable = RenderableComponent::from_cpu_mesh_handle(mesh, MaterialHandle::TOON_MESH);
universe.attach(transform, renderable);
```

The exact API name in `RenderAssets` is up to implementation, but it should express cached lookup rather than unconditional registration.
