# Task: stable material instances and material-family contracts

Date: 2026-10-10
Status: proposed internal prerequisite; documentation only.
Parent: [custom materials epic](epic/custom-materials.md).
Consumer: [Material component/resolution](material-component-and-resolved-contract.md).
First custom-program proof: [f32 time animation](material-f32-time-animation-first-slice.md).

## Goal and current prototype

Give mutable material state stable handle identity, and define how a material
family/base declares its standard per-renderable inputs and renderer contracts.
Complete the minimal internal foundation before adding custom shader programs.
Reuse Anime as the built-in proof; do not rewrite every material family first.

Anime already approximates this model: an authored Shading source owns settings,
RenderableSystem copies effective `AnimeShadingParams` to VisualInstances,
batches distinguish values, and the renderer caches their UBO/descriptor sets.
There is no stable generic material-instance record tying these layers together.

The current descriptor cache uses a HashMap keyed by material, texture/filtering,
quantization and exact Anime parameter bits. Lookup is average O(1), not a linear
search of all material values. The concerns are repeated value-key work and
allocations for each new animated value; eviction also scans for the oldest
entry. Cache capacity bounds retained entries, not allocation rate or resources
kept alive by in-flight submissions. Profile actual lookup cost before claiming
hashing itself is the dominant problem.

## Identity, storage, and draw lookup

Use proposed separate identities:

- `MaterialFamilyId`: shared interface/input/pass contract and defaults.
- `MaterialDefinitionId`: one validated built-in/custom program selection,
  family contract, supported state, and additional material input schema.
- `MaterialInstanceId`: stable mutable values/resource bindings for one definition.
- The existing renderable `InstanceHandle`: geometry/transform plus standard
  per-renderable properties and its resolved MaterialInstanceId.

Prefer generation-checked arena/slot handles for material instances and
definitions; retained draw records resolve those handles to already selected
GPU bindings/offsets with constant-time indexed lookup. No raw long-lived pointer
into a resizable Vec and no material-value hashing on every animated draw.
Names/schema field lookups can resolve to field IDs at binding time. Program/
pipeline registration and structural cache lookup may still use hashing.

Material instance identity does not change when a threshold or time value
changes. Two authored instances remain independently mutable even if their
current values happen to match. Sharing one source explicitly shares updates.
Optional immutable-value deduplication must never alias independently mutable
instances. This differs from the current value cache's opportunistic reuse.

Dirty tracking uploads values to bounded frame-safe slots; it must not overwrite
data referenced by an in-flight command buffer. Descriptor sets may be reused
with safe buffer slots/offsets or allocated in a bounded per-frame scheme. A
material instance is a logical record, not a requirement for one dedicated
descriptor set per primitive. Freeze the actual storage strategy after measuring
the existing cache workload and target Vulkan constraints.

Batch records use definition/resolved-pipeline and material-instance/binding
identity rather than all changing float bits. Distinct mutable material bindings
can split batches; per-renderable vertex/instance fields such as color need not
split a batch if its existing instanced layout supports them. Measure the
tradeoff before introducing a material-indexed buffer as another prerequisite.

## Standard instance inputs versus custom material inputs

The current renderer already supplies color, opacity, emissive intensity and
other fields through its instance data. The mesh vertex interface forwards
them to fragment varyings. Their data is physically present even when a shader
does not use it: Anime declares `v_emissive` but currently does not apply it.
Keep these fields separate from shared material uniforms such as Anime thresholds
or an authored `time_seconds` input.

| Input category | Example | Owner/update behavior |
| --- | --- | --- |
| Renderer global/per-view | camera, lights | Renderer; readonly to material instances |
| Standard per-renderable | color, opacity, emissive intensity | Renderable instance; one compatible batch can contain different values |
| Shared material parameters | Anime thresholds/shade tint, custom time_seconds | Material instance; shared consumers observe the same retained revision |
| Material resources | texture/sampler bindings | Declared owner and compatible descriptor identity; not a scalar value |

A material-family/base contract declares which standard inputs it consumes,
their types/semantics/defaults, compatible vertex/varying/descriptor interfaces,
supported state profiles, and renderer capabilities/pass implementations.
The definition selects actual surface programs and adds its custom inputs.
Start with named validated profiles; keep compatible features composable so
every subset does not become another Toon/Anime/emissive/skinned enum variant.
This is contract/preset reuse, not Rust inheritance or automatic shader-source
inheritance. Built-in models are definitions using those contracts.

For Phase 1, reuse a fixed versioned mesh-instance layout and declare supported
input subsets in metadata. Do not dynamically rebuild instance buffers for
every subset. Validate program interfaces against that layout/contract; retain
unsupported/ignored input diagnostics and existing behavior deliberately.
The f32-only restriction applies to newly authored custom uniform inputs;
existing engine-provided color vectors/matrices are still part of the interface.

## Emission: data consumption and pass participation are distinct

Reading `emissive_intensity` does not automatically opt a custom material into
emission extraction or Bloom. It may use that value only to alter main-color
shading. Keep three independent declarations/decisions:

1. **Input consumption:** the surface program uses standard emissive intensity.
2. **Extraction support/participation:** a validated emission output/pass variant
   exists and the resolved material opts into the supported extraction route.
3. **Renderer activation:** the scene enables EmissivePass/Bloom and the draw is
   eligible in its phase/view with appropriate depth/coverage/clip behavior.

A family/base can bundle sensible defaults, but all three meanings remain
explicit. Positive intensity alone must not create a pass or select Toon shaders.
An emission-capable definition can have zero intensity on some instances;
animated patterns must evaluate consistently in main color and extraction.
Opacity likewise supplies data without alone defining alpha mode/depth policy.

Implement the metadata distinction first. Generalized emission pipeline/classifier
migration and the LED extraction shader remain in the
[emission capability task](material-emission-capabilities-and-led-strip.md).
Do not make that full migration block the opaque f32 proof.

## First refactoring slices and gates

1. [ ] Inventory field owners, Anime value/cache/batch flow, shader interfaces,
   and actual input/pass consumption. Document current ignored fields and
   material-handle classification rather than assume generic behavior.
2. [ ] Define family/definition/instance IDs and records with generation-safe
   lifecycle, source sharing, defaults, resource ownership, and dirty revisions.
   Publish the fixed standard-instance interface and family metadata contract.
3. [ ] Adapt Anime through a compatibility bridge: authored Shading sources map
   to stable logical material instances; generated primitives retain source
   identity; local settings remain independent. Keep existing appearance.
4. [ ] Introduce indexed resolved draw bindings and bounded frame-safe parameter
   storage. Verify animated Anime scalar edits without creating one persistent
   UBO/descriptor per new value. Compare cache/lookup/allocation/batch costs.
5. [ ] Add the opaque mesh family/definition needed by the custom f32 slice,
   using the same instance/update machinery. Declare emission metadata without
   inferring extraction from consumption of a standard input.

Gate: two Anime sources with different settings retain shared compatible shader
pipelines and independently mutable uniform state; one source styles multiple
renderables correctly. Standard per-renderable color stays independent of shared
thresholds. Sustained edits keep handle identity and bounded GPU resources;
removal/reuse rejects stale handles and preserves in-flight resources. Built-in
fallback handles remain compatible, and no custom shader loader is needed to
prove these internal changes.

Gate for family semantics: an illustrative custom program may consume emissive
intensity without participating in extraction. The emission-capable LED variant
adds explicit pass support/participation later. Neither case gets classified by
Toon handle identity. Record unsupported combinations instead of inventing
generic shader behavior from field presence.
