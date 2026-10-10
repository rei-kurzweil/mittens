# Materials v2: shader programs, vertex-family resolution, and typed parameters

Date: 2026-08-31

Status: proposed planning epic

2026-10-10 staging update: [f32-only Material time-animation](../material-f32-time-animation-first-slice.md)
is the first testable phase. The type list and broader migration phases below
describe the longer-term architecture. Implement minimal opaque static custom
fragment/f32 support before full vertex-family migration, richer parameters,
emission, and animation-system integration.

Authoring update (2026-10-09): [Custom materials and renderer capabilities](custom-materials.md)
is the current integration epic for [0.10.0](0.10.0/README.md). `Material` is
now the intended public anchor for the full material contract; `Shading` is
built-in surface configuration within that contract. This supersedes the
September exclusion of a public Material component below. The definition,
instance, schema, vertex-family, and pipeline architecture here remains input
to that work; historical syntax is not implemented API.

Authoring update (2026-09-08): [Unified Shading and cascade](../shading-model-components-and-cascade.md)
defines the target public API: `Shading.anime()` (the default), `.toon()`,
`.unlit()`, `.refraction()`, and `.rough_transmission()` for built-ins, with
`Shader` reserved for custom shading. Both participate in the same cascade.
The `Material` MMS examples below are historical illustrative syntax, not a
proposal for an additional public component. Internal material definitions and
instances remain applicable.

## Purpose

Replace the current concrete `MaterialHandle` cross-product with a material
definition that names its fragment-stage program, resolves the compatible
vertex stage automatically from mesh capabilities, and exposes validated shader
parameters to both Rust and MMS.

This epic deliberately separates three concerns:

1. **Program intent** — what fragment program shades the surface.
2. **Geometry realization** — which static, cached-deformed/skinned, grid, or
   other compatible vertex program feeds that fragment interface.
3. **Dynamic values** — typed UBO and push-constant values, updated without
   rebuilding geometry or recompiling unrelated pipelines.

The result must let ordinary MMS authors choose a fragment program without
knowing whether a mesh is skinned. Expert MMS/Rust users may explicitly supply
both shader programs, but that is a validated override rather than the normal
authoring path.

## Related work

- [Transmissive materials: refraction and rough transmission](transmissive-materials.md)
- [Animated shader-material inputs](../animated-shader-material-inputs-mms-animation-system.md)
- [Mirror dedicated shader refactor](../mirror-dedicated-shader-refactor.md)
- [Render-to-texture specification](../../spec/render-to-texture.md)

## Decision summary

1. A material definition names a fragment program and a render-state contract.
2. The renderer selects the concrete vertex program from a declared compatible
   vertex family plus the mesh's actual capabilities. Static versus
   cached-deformed/skinned is therefore not an authored fragment-material
   variant.
3. The default authoring path is fragment-only. Explicit vertex-plus-fragment
   program selection is available as an advanced override and receives stricter
   validation.
4. Shader parameters are named, typed fields in registered schemas; MMS and
   Rust never write arbitrary bytes into UBOs or push constants.
5. Pipeline identity is a cached renderer product. Material-instance values do
   not change pipeline identity unless the schema explicitly marks a value as a
   specialization/pipeline parameter.
6. Renderer-owned globals—camera, lights, viewport, scene color, frame time—are
   not user-overridable material fields.

## Target model

The names are illustrative; the ownership is the contract.

```rust
pub struct MaterialDefinition {
    pub fragment: FragmentProgramId,
    pub vertex_family: VertexFamilyRequest,
    pub render_state: MaterialRenderState,
    pub parameters: MaterialParameterSchemaId,
}

pub enum VertexFamilyRequest {
    AutoMeshSurface,
    Explicit(VertexProgramId), // advanced override
}

pub struct MaterialInstance {
    pub definition: MaterialDefinitionId,
    pub values: MaterialParameterValues,
}
```

For ordinary mesh materials, `AutoMeshSurface` resolves approximately as:

```text
fragment program requires MeshSurfaceV1
  + mesh has no cached deformation -> mesh-surface-static vertex program
  + mesh has cached deformation    -> mesh-surface-cached-deformed vertex program
  + unsupported mesh capability    -> validation error before draw
```

The resolved Vulkan pipeline key additionally includes the fragment program,
resolved vertex program, render state/phase, clip variant, target formats, and
MSAA. It does not include ordinary UBO/push-constant values.

## Program interfaces and validation

Each built-in or registered program declares:

- a versioned vertex-input and vertex-varying interface;
- descriptor-set requirements and ownership (`renderer global`, `material`,
  `instance`, or `per-view`);
- its parameter schema;
- compatible render phases and blend/depth/raster constraints; and
- whether it can consume a static, cached-deformed, grid, or explicit vertex
  source.

When only a fragment program is selected, the renderer chooses a compatible
vertex implementation. When both programs are explicitly selected, validation
must reject mismatched varyings, missing attributes, incompatible descriptors,
and invalid render state before pipeline creation.

The first version supports a registry of known programs and schemas. Arbitrary
unrestricted shader files, reflection-only discovery, and a public shader
plug-in ABI are later decisions, not accidental side effects of this epic.

## Authoring surface

### Default: fragment program plus automatic vertex family

Illustrative MMS:

```mms
let glass = Material.fragment("refraction_mesh") {
    state("transmissive")
    uniform.float("ior", 1.48)
    uniform.float("thickness", 0.20)
}

R.sphere() { glass }
```

This must use the same authored `glass` definition for static and cached-
deformed meshes, while selecting their concrete vertex programs internally.

### Advanced: explicit program pair

```mms
let diagnostic = Material.programs(
    vertex = "mesh_surface_static_v1",
    fragment = "normal_diagnostic_v1",
) {
    state("opaque")
}
```

This opt-out is technical and intentional. It is valid only when the selected
vertex program supports the receiving mesh and its interface matches the
fragment program. It does not silently disable skinning/deformation; applying a
static-only override to a cached-deformed mesh is an actionable authoring
error. Rust receives an equivalent explicit builder/API.

Exact constructor spelling can evolve, but preserve this split between the
safe default and a visible expert override.

## Typed dynamic parameter model

Programs expose a schema of named fields with one storage class:

| Storage class | Ownership and update rule | Initial use |
| --- | --- | --- |
| Renderer global / per-view UBO | Renderer writes it each frame; readonly to MMS/Rust material instances. | camera, viewport, lights, scene color, time |
| Shared material UBO | One definition/instance value shared by many draws; update/upload when changed. | palette, global shader tuning |
| Per-instance UBO or storage record | One renderable override; can split batches only where its chosen storage requires it. | refraction IOR/thickness, per-object color |
| Push constants | Small, pipeline-layout-declared values written at draw time; typed and bounded. | a small draw-local control, once justified |
| Descriptor field | Typed resource handle with explicit lifecycle; not a raw numeric UBO value. | authored texture/sampler later |

MMS and Rust setters operate on declared field names and declared value types:

```text
bool, int, uint, float, vec2, vec3, vec4, color, mat3, mat4
```

Arrays, structs, textures, specialization constants, and storage-buffer access
need explicit follow-up contracts. Each field declares default, range/finite
validation where applicable, storage class, animation behavior, and whether a
change dirties an upload, descriptor set, batch, or pipeline.

### Push-constant rule

Push constants are not an unstructured escape hatch. A program may expose them
only through schema fields that fit the resolved pipeline layout's declared
range. The renderer packs and validates them. If a value must vary per instance
and harms batching as a push constant, move it to the per-instance record
instead of exposing raw push-constant offsets to MMS.

## Rust ownership and updates

Rust needs APIs parallel to MMS:

```rust
let definition = materials.define_fragment(FragmentProgramId::Refraction, state)?;
let instance = materials.instantiate(definition)?;
instance.set_float("ior", 1.48)?;
instance.set_vec4("tint", [0.86, 0.96, 1.0, 1.0])?;
```

The API validates schema and routes the smallest necessary GPU update. Material
definitions and instances have stable identities across frames; updates are
dirty-tracked and applied at a renderer-owned synchronization point. No caller
directly mutates a mapped UBO or push-constant byte range.

## Execution plan

### Phase 0: inventory and compatibility bridge

- [ ] Inventory every `MaterialHandle` match, UBO layout, descriptor set,
      pipeline creation path, batching key, render phase, and shader interface.
- [ ] Define built-in program IDs, interface IDs, vertex-family requirements,
      render-state constraints, and parameter-schema metadata.
- [ ] Keep existing material handles working through a compatibility resolver;
      do not rewrite every scene at once.

Exit gate: all current materials have a documented mapping into the new
registry shape, with no behavior change.

### Phase 1: fragment selection and automatic vertex resolution

- [ ] Implement `MaterialDefinition` with fragment selection and
      `AutoMeshSurface` resolution.
- [ ] Resolve static versus cached-deformed vertex programs from actual mesh
      capabilities, not authored `SKINNED_*` material names.
- [ ] Implement the explicit vertex-plus-fragment override with validation.
- [ ] Move pipeline lookup behind one authoritative resolved-pipeline key.
- [ ] Migrate toon, emissive, mirror, and sharp refraction as proof cases.

Exit gate: one fragment definition draws valid static and cached-deformed meshes
without authored geometry variants; invalid explicit overrides fail clearly.

### Phase 2: parameter schemas and shared material UBO updates

- [ ] Register typed schemas for the migrated built-in programs.
- [ ] Add Rust definition/instance creation and setters with validation.
- [ ] Add MMS constructors, typed fields, setters, serialization, and reload
      behavior.
- [ ] Implement shared-material UBO allocation, dirty upload, and batching
      behavior.
- [ ] Keep renderer global/per-view inputs readonly and separately owned.

Exit gate: Rust and MMS can update a scalar and a vector in a shared material
without pipeline recreation or unbounded descriptor churn.

### Phase 3: per-instance values and push constants

- [ ] Define per-renderable overrides and their batch/update behavior.
- [ ] Add a small schema-backed push-constant path with layout validation.
- [ ] Decide which existing values stay in instance data versus move to the
      material parameter system.
- [ ] Integrate setters with `Animation`/`Keyframe` and benchmark many updates.

Exit gate: an animated field updates through MMS and Rust with predictable GPU
cost and no raw-byte authoring API.

### Phase 4: transmission and extension migration

- [ ] Replace temporary `REFRACTION_MESH` / `SKINNED_REFRACTION_MESH` handles
      with one refraction fragment definition plus automatic vertex selection.
- [ ] Add rough transmission only through this resolver and schema path.
- [ ] Decide texture/descriptors, arrays, and user-supplied shader programs from
      measured needs rather than adding a general node graph.

## Acceptance criteria

- Fragment program selection is normal MMS/Rust authoring.
- Static and cached-deformed vertex implementations are renderer-selected when
  the selected fragment interface permits both.
- Explicit program pairs are possible but validated.
- Every exposed UBO or push-constant field is named, typed, range-checked, and
  has declared ownership/update cost.
- Parameter updates do not recreate meshes or unrelated pipelines.
- Existing built-in scenes continue to render through the compatibility bridge.
- The transmission epic no longer owns the general material architecture; it
  consumes Materials v2.

## Non-goals

- arbitrary raw GPU-buffer writes from MMS;
- unvalidated arbitrary shader pairing;
- a node-graph material editor;
- broad shader hot reload/migration policy beyond registered definitions; and
- refactoring every material family before the compatibility bridge is proven.
