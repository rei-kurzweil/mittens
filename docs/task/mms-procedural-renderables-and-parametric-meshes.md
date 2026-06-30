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

And it should establish the pattern for additional rotational procedural
families:

- `R.cylinder(circle_segments, height_segments)`
- `R.partial_cylinder(max_circle_segments, height_segments, angular_distance_radians)`
- `R.torus(major_segments, minor_segments)`
- `R.partial_torus(max_major_segments, minor_segments, angular_distance_radians)`
- `R.spring(max_major_segments, minor_segments, angular_distance_radians, radians_per_unit_height)`

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
   - cylinder / partial cylinder
   - torus / partial torus
   - spring
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

T.position(0.0, 0.0, -4.0) {
    R.cylinder(48, 8) {
        C.rgba(0.8, 0.85, 0.95, 1.0)
    }
}

T.position(0.0, 0.0, -4.0) {
    R.partial_cylinder(48, 8, 1.5707963) {
        C.rgba(0.95, 0.55, 0.25, 1.0)
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
- `cylinder(render_assets, circle_segments, height_segments)`
- `partial_cylinder(render_assets, max_circle_segments, height_segments, angular_distance_radians)`
- `torus(render_assets, major_segments, minor_segments)`
- `partial_torus(render_assets, max_major_segments, minor_segments, angular_distance_radians)`
- `spring(render_assets, max_major_segments, minor_segments, angular_distance_radians, radians_per_unit_height)`

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
- `cylinder`
- `partial_cylinder`
- `torus`
- `partial_torus`
- `spring`

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

It likely also wants a reusable shape-family enum rather than one ad hoc field
per constructor, because the rotational families below share a lot of topology
rules and save/load semantics.

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

## Cylinder / partial cylinder

Add a 3D cylinder family in `MeshFactory`.

Target authored APIs:

- `R.cylinder(circle_segments, height_segments)`
- `R.partial_cylinder(max_circle_segments, height_segments, angular_distance_radians)`

Requirements:

- axis-aligned default orientation should be specified and kept consistent with
  the rest of `MeshFactory`
- support configurable radial tessellation and vertical tessellation
- include side surface topology
- decide explicitly whether v1 includes end caps; if omitted, document that
  clearly rather than leaving it ambiguous

### Partial-cylinder angular sampling rule

The partial-cylinder constructor needs a precise sampling rule:

- `max_circle_segments` means the segment density the full cylinder would have
  around `2π`
- `angular_distance_radians` is the exact angular extent to realize
- the generated arc may use up to roughly
  `full_circle_segment_count * (angular_distance / 2π)`, rounded up
- this can produce a final radial boundary that is not exactly located at the
  next evenly spaced full-circle segment position
- that is acceptable and intentional if it is needed to hit the exact requested
  angular distance

In other words:

- density should be inherited from the full-circle tessellation budget
- extent should still land exactly on the requested terminal angle
- the last segment may be narrower than the earlier ones

This same question likely applies to `partial_annulus_2d(...)` and is worth
auditing there rather than treating cylinder as a special case by accident.

## Torus / partial torus

Add a torus family in `MeshFactory`.

Target authored APIs:

- `R.torus(major_segments, minor_segments)`
- `R.partial_torus(max_major_segments, minor_segments, angular_distance_radians)`

Requirements:

- full torus should represent a closed major loop with a circular minor cross
  section
- partial torus should reuse the same major-loop density logic as
  `partial_cylinder`
- the exact requested angular extent should be honored even if the final major
  segment is shorter than the regular full-torus spacing
- decide explicitly whether the ends of a partial torus are open or capped in v1

## Spring

Add a spring / helix-tube family in `MeshFactory`.

Target authored API:

- `R.spring(max_major_segments, minor_segments, angular_distance_radians, radians_per_unit_height)`

Intended behavior:

- topologically similar to a `partial_torus`
- unlike a torus, total angular distance may exceed `2π`
- as the shape winds, `y` increases according to a pitch control

Parameter meaning:

1. `max_major_segments`
   - segment-density budget per `2π` of winding

2. `minor_segments`
   - cross-section segmentation around the tube

3. `angular_distance_radians`
   - total wound angle
   - may be larger than `2π`

4. `radians_per_unit_height`
   - pitch control
   - how many radians of winding correspond to `+1.0` unit of `y`

This should likely be interpreted so that:

- `delta_y = angular_distance_radians / radians_per_unit_height`

unless implementation experience shows that the inverse convention reads more
naturally in code or authoring. The important part is to document one convention
clearly and keep it consistent.

## Shared geometry seam across full and partial rotational shapes

We should avoid implementing:

- annulus
- partial annulus
- cylinder
- partial cylinder
- torus
- partial torus
- spring

as unrelated one-off mesh generators.

There is likely a reusable internal seam for:

- angular sampling over a requested extent
- density budget derived from the "full" shape segment count
- exact terminal-angle placement for partial variants
- optional seam closure for full variants
- optional cap generation for open-ended variants
- cross-section sweep along a path

This task should explicitly look for shared helpers rather than copying the same
arc-sampling logic into every shape family.

At minimum, the code should try to centralize:

- angular sampling for full vs partial sweeps
- terminal-angle handling when exact extent does not land on an evenly spaced
  nominal segment boundary
- sweep-path tessellation utilities

## RenderAssets / built-in policy

We need to be precise about terminology here.

These shapes are "built in" in the sense that the engine knows how to generate
them, but they are not all fixed singleton mesh handles like `Cube` or
`Circle2D`.

The star, heart, cylinder, torus, spring, and partial variants are parameterized
families, so they should be treated as:

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
    Cylinder {
        circle_segments: u32,
        height_segments: u32,
    },
    PartialCylinder {
        max_circle_segments: u32,
        height_segments: u32,
        angular_distance_bits: u32,
    },
    Torus {
        major_segments: u32,
        minor_segments: u32,
    },
    PartialTorus {
        max_major_segments: u32,
        minor_segments: u32,
        angular_distance_bits: u32,
    },
    Spring {
        max_major_segments: u32,
        minor_segments: u32,
        angular_distance_bits: u32,
        radians_per_unit_height_bits: u32,
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

- [x] `R.partial_annulus_2d(...)` can be authored directly in `.mms`
- [x] `R.star(...)` can be authored directly in `.mms`
- [x] `R.heart(...)` can be authored directly in `.mms`
- [ ] `R.cylinder(...)` can be authored directly in `.mms`
- [ ] `R.partial_cylinder(...)` can be authored directly in `.mms`
- [ ] `R.torus(...)` can be authored directly in `.mms`
- [ ] `R.partial_torus(...)` can be authored directly in `.mms`
- [ ] `R.spring(...)` can be authored directly in `.mms`
- [ ] invalid constructor names fail clearly
- [ ] invalid argument counts fail clearly
- [ ] invalid argument ranges fail clearly

### Mesh generation

- [x] partial annulus generates correct open-arc annulus geometry
- [x] star generates stable filled topology for sharp corners
- [x] star generates stable filled topology for beveled outer points
- [x] star generates stable filled topology for beveled inner valleys
- [x] heart generates a stable filled silhouette across practical segment counts
- [ ] cylinder generates stable side topology across practical radial and height segment counts
- [ ] partial cylinder honors exact terminal angle while preserving sensible segment density
- [ ] torus generates stable closed topology
- [ ] partial torus honors exact terminal angle while preserving sensible segment density
- [ ] spring generates stable helical topology for angles above and below `2π`

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
6. Add `MeshFactory::cylinder(...)` and `MeshFactory::partial_cylinder(...)`.
7. Add `MeshFactory::torus(...)`, `MeshFactory::partial_torus(...)`, and `MeshFactory::spring(...)`.
8. Add MMS constructors for all new procedural renderables.
9. Add save / round-trip coverage.
10. Add example scenes authored fully in MMS.

## Completion criteria

This task is complete when:

- MMS can author `R.partial_annulus_2d(...)`, `R.star(...)`, `R.heart(...)`,
  `R.cylinder(...)`, `R.partial_cylinder(...)`, `R.torus(...)`,
  `R.partial_torus(...)`, and `R.spring(...)`
- the procedural meshes are registered during normal MMS spawning without Rust
  hand-wiring per instance
- authored procedural renderables save and reload correctly
- the new shape families have stable mesh-generation behavior and validation
