# Task: MMS Procedural Renderables And Parametric Meshes

Date: 2026-06-30

Status: proposed

## Goal

Allow MMS to author parameterized procedural meshes directly via `Renderable`
constructors, instead of requiring Rust-side `render_assets.register_mesh(...)`
calls for every custom shape.

The immediate motivating case is:

- `R.partial_annulus_2d(inner_radius, outer_radius, start_angle_radians, sweep_angle_radians, segments)`

This task also adds two new procedural mesh families:

- `R.star(points, inner_radius_fraction, outer_bevel_segments, inner_bevel_segments)`
- `R.heart(segments)`

The intended end state is that these shapes are authored directly in `.mms`
files and instantiated by the normal MMS component materialization path.

## Current problem

Today MMS `Renderable` creation is limited to a small fixed set of shared
built-in mesh constructors:

- `cube`
- `circle2d`
- `sphere`
- `triangle`
- `square`
- `plane`
- `tetrahedron`

That path is hard-coded in:

- [src/meow_meow/component_registry.rs](../../src/meow_meow/component_registry.rs)

The current MMS spawn path only receives:

- `World`
- `SignalEmitter`

It does not receive:

- `RenderAssets`

That means MMS can instantiate pre-existing mesh handles, but it cannot create a
new CPU mesh and register it at spawn time for parameterized shapes like partial
annuli, stars, or hearts.

There is a second gap: dynamically registered meshes do not currently preserve
enough authored provenance to round-trip back into MMS source cleanly.

## Scope

This task covers:

1. MMS-side authoring of parameterized procedural renderables.
2. The runtime seam needed for MMS spawning to register CPU meshes into
   `RenderAssets`.
3. Procedural mesh generation for:
   - partial annulus
   - star
   - heart
4. Round-trip / serialization behavior for those renderables.

## Non-goals

- arbitrary user-defined mesh scripting in MMS
- introducing a generalized geometry DSL
- runtime mesh deformation after spawn
- material-system redesign
- glTF or imported-mesh authoring changes

## Required authoring surface

The target authoring syntax should look like:

```mms
T.position(0.0, 0.0, -4.0) {
    R.partial_annulus_2d(0.55, 0.89, 0.0, 1.5707963, 48) {
        C.rgba(1.0, 0.0, 0.0, 1.0)
    }
}

T.position(0.0, 0.0, -4.0) {
    R.star(5, 0.45, 3, 2) {
        C.rgba(1.0, 0.9, 0.2, 1.0)
    }
}

T.position(0.0, 0.0, -4.0) {
    R.heart(64) {
        C.rgba(1.0, 0.2, 0.45, 1.0)
    }
}
```

Exact naming can still be tightened, but the core requirement is:

- the shape constructor and its parameters belong in MMS
- Rust should not need to hand-register each instance

## Proposed implementation direction

## 1. Thread `RenderAssets` through MMS spawn/materialization

The MMS component materialization path needs access to `RenderAssets` so that
`create_component(...)` can register parameterized meshes during spawn.

Likely affected surfaces:

- `spawn_tree(...)`
- `spawn_tree_uninitialized(...)`
- `create_component(...)`

This is the main missing seam for `R.partial_annulus_2d(...)`.

## 2. Extend `RenderableComponent` with procedural constructors

Add explicit constructors on `RenderableComponent` for procedural authored
meshes, likely in the same style as the existing `*_dynamic(...)` helpers.

At minimum:

- `partial_annulus_2d(render_assets, inner, outer, start, sweep, segments)`
- `star(render_assets, points, inner_radius_fraction, outer_bevel_segments, inner_bevel_segments)`
- `heart(render_assets, segments)`

These constructors should:

- generate a `CpuMesh` through `MeshFactory`
- register it in `RenderAssets`
- construct a `RenderableComponent`
- preserve a meaningful `base_mesh` / provenance story for downstream systems

## 3. Add MMS constructor cases

Extend the `Renderable` branch in `create_component(...)` to support the new
constructor names and arguments.

At minimum:

- `partial_annulus_2d`
- `star`
- `heart`

These constructors should validate arguments and return good authoring errors.

## 4. Preserve authored provenance for save / round-trip

This is not optional if we want authored procedural renderables to behave like
real MMS-authored scene content rather than one-way runtime artifacts.

The current `RenderableComponent::to_mms_ast(...)` only maps a small fixed set
of known shared mesh handles back to constructor calls. Procedural shapes need a
new representation that retains:

- constructor name
- constructor arguments

Recommended direction:

- store an authored renderable descriptor on `RenderableComponent`
- use that descriptor in `to_mms_ast(...)`

Possible shape:

```rust
enum AuthoredRenderableShape {
    Builtin { ctor: &'static str },
    PartialAnnulus2d {
        inner_radius: f32,
        outer_radius: f32,
        start_angle_radians: f32,
        sweep_angle_radians: f32,
        segments: u32,
    },
    Star {
        points: u32,
        inner_radius_fraction: f32,
        outer_bevel_segments: u32,
        inner_bevel_segments: u32,
    },
    Heart {
        segments: u32,
    },
}
```

The exact type shape can change, but the engine needs this kind of authored
descriptor somewhere.

## Mesh requirements

## Partial annulus

`MeshFactory::partial_annulus_2d(...)` now exists and should become reachable
from MMS authoring.

Requirements:

- XY-plane mesh
- configurable inner radius
- configurable outer radius
- configurable start angle
- configurable sweep angle
- configurable segment count
- stable winding and UV behavior consistent with existing 2D shapes

## Star

Add a new procedural star mesh in `MeshFactory`.

Target authored API:

- `R.star(points, inner_radius_fraction, outer_bevel_segments, inner_bevel_segments)`

Parameter semantics:

1. `points`
   - integer
   - number of outer points of the star
   - minimum should be validated sensibly, likely `>= 3`

2. `inner_radius_fraction`
   - float
   - fraction of the outer radius used by the inward concave vertices
   - expected range should likely be clamped or validated to `(0, 1]`

3. `outer_bevel_segments`
   - integer
   - number of subdivisions used to round each outer tip

4. `inner_bevel_segments`
   - integer
   - number of subdivisions used to round each concave inner valley

Behavior expectations:

- the base shape is a 2D filled star in the XY plane
- outer points should be optionally rounded by replacing the sharp tip with an
  arc-like bevel made of `outer_bevel_segments`
- concave valleys should be optionally rounded with `inner_bevel_segments`
- `0` bevel segments should mean sharp corners
- UVs should remain reasonable and stable for simple texturing/gradients
- winding should remain CCW for the front face

Open design question:

- whether bevel rounding should use true circular arcs or a simpler interpolated
  corner approximation

For v1, consistency and stable topology matter more than mathematically perfect
curvature.

## Heart

Add a new procedural heart mesh in `MeshFactory`.

Target authored API:

- `R.heart(segments)`

Requirements:

- XY-plane filled heart mesh
- only one authored control: `segments`
- no extra shape parameters in v1
- the silhouette should be stable and recognizably heart-shaped at ordinary
  segment counts
- winding should remain CCW for the front face
- UVs should map cleanly across the shape bounds

Recommended implementation shape:

- define a standard 2D heart curve / contour
- sample it by `segments`
- triangulate into a filled polygon mesh

## RenderAssets / built-in policy

We need to be precise about terminology here.

These shapes are "built in" in the sense that the engine knows how to generate
them, but they are not all fixed singleton mesh handles like `Cube` or
`Circle2D`.

The star and heart are parameterized families, so they should be treated as:

- engine-supported procedural mesh constructors
- dynamically registered CPU meshes at spawn time

Not as:

- one global singleton handle per shape family regardless of parameters

If we later want caching or deduplication, that should be keyed by the full
parameter tuple rather than by shape name alone.

## Suggested runtime data model

If repeated authored shapes are common, `RenderAssets` may want a procedural-mesh
cache keyed by shape descriptor, for example:

```rust
enum ProceduralMeshKey {
    PartialAnnulus2d {
        inner_radius_bits: u32,
        outer_radius_bits: u32,
        start_angle_bits: u32,
        sweep_angle_bits: u32,
        segments: u32,
    },
    Star {
        points: u32,
        inner_radius_fraction_bits: u32,
        outer_bevel_segments: u32,
        inner_bevel_segments: u32,
    },
    Heart {
        segments: u32,
    },
}
```

This is not strictly required for the first pass, but it is the likely correct
direction once procedural authoring exists.

## Systems impact

The new authored shapes may affect:

- `RenderableComponent` construction
- MMS component registry/materialization
- scene save / MMS round-trip
- bounds inference
- raycast shape inference if any of these should get special handling later

Notes:

- partial annulus may still use generic mesh bounds even if raycast later wants
  an analytic ring/arc shape
- star and heart can start with ordinary triangle mesh rendering without any
  special narrow-phase picking support

## Validation checklist

### MMS authoring

- [ ] `R.partial_annulus_2d(...)` can be authored directly in `.mms`
- [ ] `R.star(...)` can be authored directly in `.mms`
- [ ] `R.heart(...)` can be authored directly in `.mms`
- [ ] invalid constructor names fail clearly
- [ ] invalid argument counts fail clearly
- [ ] invalid argument ranges fail clearly

### Mesh generation

- [ ] partial annulus generates correct open-arc annulus geometry
- [ ] star generates stable filled topology for sharp corners
- [ ] star generates stable filled topology for beveled outer points
- [ ] star generates stable filled topology for beveled inner valleys
- [ ] heart generates a stable filled silhouette across practical segment counts

### Round-trip / serialization

- [ ] saving authored procedural shapes preserves their constructor names
- [ ] saving preserves constructor arguments
- [ ] reloading reproduces the same shape

### Asset/runtime behavior

- [ ] repeated identical authored shapes do not explode mesh count unnecessarily if caching is added
- [ ] bounds are valid for all three shape families
- [ ] renderables appear correctly with color, emissive, texture, and UV modifiers

## Suggested implementation order

1. Thread `RenderAssets` through MMS spawn/materialization.
2. Add authored-provenance storage for `RenderableComponent`.
3. Expose `partial_annulus_2d` from MMS using the existing mesh factory support.
4. Add `MeshFactory::star(...)`.
5. Add `MeshFactory::heart(...)`.
6. Add MMS constructors for `star` and `heart`.
7. Add save / round-trip coverage.
8. Add example scenes authored fully in MMS.

## Completion criteria

This task is complete when:

- MMS can author `R.partial_annulus_2d(...)`, `R.star(...)`, and `R.heart(...)`
- the procedural meshes are registered during normal MMS spawning without Rust
  hand-wiring per instance
- authored procedural renderables save and reload correctly
- the new shape families have stable mesh-generation behavior and validation
