# Task: stable material instances and MaterialBase contracts

Date: 2026-10-10
Status: built-in Anime/Toon instance foundation accepted in VR; generic parameter/base-contract migration pending.

2026-10-24: [the custom-f32 room checkpoint](youtuber-room-custom-material-checkpoint.md)
adds MMS definition/instance registration and reflected variable-size float
uploads. The built-in/common-flow TODO below remains open; this does not require
moving built-in definition authoring out of Rust/GLSL.
Parent: [custom materials epic](epic/custom-materials.md).
Consumer: [Material component/resolution](material-component-and-resolved-contract.md).
First custom-program proof: [f32 time animation](material-f32-time-animation-first-slice.md).
Sharing topology: [multi-parent component task](multi-parent-components-and-material-sharing.md).

2026-10-23: the user resumed implementation after accepting shared declarations
in VR. [The second checkpoint](material-instances-anime-vr-checkpoint.md) adds
generation-checked family/definition/instance records, source-linked Anime/Toon
draw bindings and bounded mutable GPU storage to the same scene. A new pulse
control exercises sustained updates. The user subsequently confirmed that the
updated scene works. Record that foundation as accepted without treating it as
completion of generic material values, authoring or renderer feature selection.

Naming decision (revised by the later MMS discussion): exactly three layers,
**MaterialBase -> MaterialDefinition -> Material**. MaterialBase is the new name
for MaterialFamily, not another layer. Material is the concrete instance;
MaterialInstance remains its current Rust name. Definitions declare field types,
instances supply values. The [authoring contract](material-component-and-resolved-contract.md#authoring-decision-2026-10-23)
records constructor overloads, default `MaterialBase()` and lowercase type tags.
Source symbols/file paths retain their existing names until a source refactor.
The [checkpoint's specialization inventory](material-instances-anime-vr-checkpoint.md)
records Anime-typed parameters/defaults, fixed upload packing and Shading-specific
resolution hooks that remain to be generalized. This pass changes docs only.

## Goal and current prototype

Give mutable material state stable handle identity, and define how a material
base declares its standard per-renderable inputs and renderer contracts.
Complete the minimal internal foundation before adding custom shader programs.
Reuse Anime as the built-in proof; do not rewrite every built-in material first.

The built-in bridge now implements this model: one initialized authored Shading
source owns a retained material instance; renderables/batches store its ID.
Anime scalar edits change one shared record/revision without copying settings
into renderables or changing batch keys. GLTF projections consume the source
record and their live getters/setters address that source.

Before this slice, the descriptor cache used a HashMap keyed by material, texture/filtering,
quantization and exact Anime parameter bits. Lookup is average O(1), not a linear
search of all material values. The concerns are repeated value-key work and
allocations for each new animated value; eviction also scans for the oldest
entry. Cache capacity bounds retained entries, not allocation rate or resources
kept alive by in-flight submissions. Profile actual lookup cost before claiming
hashing itself is the dominant problem.

Authored Anime/Toon now use indexed instance records plus a bounded structural
resource cache. Parameter values are not part of the GPU binding key. Unauthored
built-in fallback draws retain the original immutable descriptor cache.

## Identity, storage, and draw lookup

The first implementation provides these three identities under their current
source names:

- `MaterialFamilyId` (target name `MaterialBaseId`): shared interface/input/pass
  and renderer state/configuration contract.
- `MaterialDefinitionId` (the MaterialDefinition): built-in program selection and defaults
  now; validated custom programs/input schemas remain pending.
- `MaterialInstanceId` (the public Material): stable mutable values/resource bindings for one definition.

The existing renderable `InstanceHandle` identifies a drawable's geometry,
transform and standard per-renderable properties plus its selected material
instance. It is not a fourth layer in the material model.

Prefer generation-checked arena/slot handles for material instances and
definitions; retained draw records resolve those handles to already selected
GPU bindings/offsets with constant-time indexed lookup. No raw long-lived pointer
into a resizable Vec and no material-value hashing on every animated draw.
Names/schema field lookups can resolve to field IDs at binding time. Program/
pipeline registration and structural cache lookup may still use hashing.

The counting rule is simple: **one instantiated Material component owns one
logical renderer material instance**. Reusing that source shares its instance;
constructing another Material creates another instance even with identical
parameters/programs. For the Anime bridge, one authored Shading source owns
one logical material instance; generated source projections are consumers,
not additional authored instances. Do not derive identity/count from parameter
hashes or add mutable-instance value deduplication. GPU allocation can be lazy
and frame-buffered without changing this logical ownership rule.

Material instance identity does not change when a threshold or time value
changes. Program/definition/pipeline resources can be shared independently of
the separately mutable material components.

Structural ECS components remain single-parent. Leaf Shading declarations now
support multiple attachments through the linked graph task, so repeating one
live Shading handle beneath unrelated renderables shares its component identity
and settings. Wrapper inheritance also shares a source within one subtree.
The public `Material` / `MaterialDefinition` components and generic parameter
schemas remain pending. Multiple Shading attachments do not construct another
authored source; the existing instance-ID bridge preserves that ownership rule.
Broader sharing must preserve structural movement,
lifetime and performance contracts without picking an arbitrary first parent.

Dirty tracking uploads values to bounded frame-safe slots; it must not overwrite
data referenced by an in-flight command buffer. Descriptor sets may be reused
with safe buffer slots/offsets or allocated in a bounded per-frame scheme. A
material instance is a logical record, not a requirement for one dedicated
descriptor set per primitive. The initial implementation uses at most eight
UBO/descriptor snapshots per structural binding variant and retains at most 16
variants per material. Descriptor Arc ownership protects even unsubmitted
recordings; Vulkano write guards additionally protect GPU use. Pool exhaustion
returns an explicit error instead of allocating indefinitely or overwriting a
retained revision. See the checkpoint for measured allocation/readback evidence
and the remaining resource-variation/performance limits.

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

A MaterialBase contract declares which standard inputs it supports,
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

A base can bundle sensible defaults, but all three meanings remain
explicit. Positive intensity alone must not create a pass or select Toon shaders.
The [lighting/emission/extraction policy review](material-lighting-emission-and-extraction-policy.md)
adds the proposed built-in rule that intensity 0 suppresses extraction while
emissive/unlit shading remains unlit and visible. Keep lighting selection
independent, and distinguish supported extraction from effective source
selection in the resolved contract. Whole-draw omission at zero requires a
contract guaranteeing zero extracted output, not arbitrary input consumption.
An emission-capable definition can have zero intensity on some instances;
animated patterns must evaluate consistently in main color and extraction.
Opacity likewise supplies data without alone defining alpha mode/depth policy.

Implement the metadata distinction first. Generalized emission pipeline/classifier
migration and the LED extraction shader remain in the
[emission capability task](material-emission-capabilities-and-led-strip.md).
Do not make that full migration block the opaque f32 proof.

## First refactoring slices and gates

2026-10-25 input API slice: the same `.inputs(...)` method is implemented for
definition primitive-record types and instance field values, with f32-only GPU
fields. Named material inputs are
independent of their GPU transport; keep schema, ownership/rate and physical
layout separate. The current custom implementation remains f32 UBO-backed.
See [the input/storage contract](material-component-and-resolved-contract.md#named-inputs-and-gpu-transport-2026-10-25).
This adds primitive type-only records, not mixed/typed-value records or
push-constant support. Common-flow work should remove Anime-specific storage/packing assumptions
without requiring additional GPU input mechanisms first.

2026-10-24 follow-up TODO: built-in Anime/Toon definitions may remain authored
in Rust/GLSL, but must use the same material registration, initialization,
binding, retained-update and retirement logic as other material definitions.
The new custom-f32 prototype still reuses `pending_anime_shading` and retains
Anime-specific selection/parameter code in RenderableSystem. Unify that flow
through MaterialSystem and the common registry/resolver; a field rename alone
does not complete this task. Keep `Shading.anime()` / `.toon()` as compatibility
authoring adapters. This is migration work after the room checkpoint, not a
requirement to move built-in shader definitions into MMS or remove Anime source
files. The user explicitly requested recording this follow-up for now.

1. [x] Audit the Anime/Toon field owners, value/cache/batch flow, fixed interface
   and actual input/pass consumption. Document current ignored fields and
   material-handle classification rather than assume generic behavior.
2. [x] Define built-in base/material/instance IDs and records with generation-safe
   lifecycle, source sharing, defaults, resource ownership, and dirty revisions.
   Publish the fixed standard-instance interface and initial base metadata contract.
3. [x] Adapt Anime through a compatibility bridge: authored Shading sources map
   to stable logical material instances; generated primitives retain source
   identity; local settings remain independent. Keep existing appearance.
4. [x] Introduce indexed resolved draw bindings and bounded frame-safe parameter
   storage. Verify animated Anime scalar edits without creating one persistent
   UBO/descriptor per new value. Verify allocation and batch-rebuild counts;
   comprehensive CPU timing comparisons remain follow-up work.
5. [ ] Replace Anime-specific value/default storage and upload packing with
   schema-backed material data and layout. Generalize the Shading-specific
   resolution bridge; consolidate renderer state/features under MaterialBase
   and perform the agreed source renames without adding a layer.
6. [ ] Add the opaque mesh base/material support needed by the custom f32 slice,
   using the same instance/update machinery. Declare emission metadata without
   inferring extraction from consumption of a standard input.

Gate: two Anime sources with different settings retain shared compatible shader
pipelines and independently mutable uniform state; one source styles multiple
renderables correctly. Standard per-renderable color stays independent of shared
thresholds. Sustained edits keep handle identity and bounded GPU resources;
removal/reuse rejects stale handles and preserves in-flight resources. Built-in
fallback handles remain compatible, and no custom shader loader is needed to
prove these internal changes.

Verify renderer records after actual MMS initialization/mutations: two distinct
sources with equal values have distinct IDs; consumers of one source have the
same ID; mutation leaves those IDs/counts stable. When cross-subtree material
references are implemented, binding one source in two places must preserve
both bindings without reparenting it or allocating another logical instance.

Gate for base semantics: an illustrative custom program may consume emissive
intensity without participating in extraction. The emission-capable LED variant
adds explicit pass support/participation later. Neither case gets classified by
Toon handle identity. Record unsupported combinations instead of inventing
generic shader behavior from field presence.
