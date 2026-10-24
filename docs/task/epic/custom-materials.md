# Epic: custom materials, typed inputs, and renderer capabilities

Date: 2026-10-09
Status: shared declarations and built-in instance foundation accepted in VR; generic parameter/resolution migration and custom programs remain pending.
Release: [mittens-engine 0.10.0](0.10.0/README.md).

2026-10-23 authoring revision: [the MMS material contract](../material-component-and-resolved-contract.md#authoring-decision-2026-10-23)
names the three layers MaterialBase, MaterialDefinition and Material (instance).
Definition schemas use colon-style type expressions resolving lowercase
imported type identities; Material records use equals-style initial values.
This follows the later [record/type/value revision](../../../crates/meow-meow-script/docs/draft/record-types-and-values.md),
whose mixed forms and dual roles remain under review. A real `TypeExpression`
foundation precedes that generic schema syntax. The later first-slice decision
allows MMS-authored field names/count through an existing-grammar declaration
API, with every field f32, before type expressions are available. Typed numeric
defaults/precise `typeof` are separate language work. No generic authoring API
is implemented by that documentation update. The subsequent primitive MMS
tag-value slice is now implemented: lowercase imports, identity equality,
schema-table storage and lossless host/callback transport. `typeof`, numeric
annotations/defaults and the Material API remain pending. See the
[implemented tag boundary](../../../crates/meow-meow-script/docs/draft/type-system-epic.md#implemented-tag-value-boundary-2026-10-23).

2026-10-10 first-phase decision: [f32 time-animation proof](../material-f32-time-animation-first-slice.md)
is the first testable custom-program slice, after the internal instance/family
foundation below. Authored shader inputs support
only finite `f32` GPU fields initially. Beyond the type-tag prerequisite,
broader MMS type integration, additional input types/providers, and renderer
emission follow after that proof.

## Outcome

Make `Material` the authored component that anchors a complete material
contract: surface program, typed inputs, compatible geometry interfaces,
render state, and participation in renderer features such as emissive extraction.
Built-in and custom materials resolve through the same high-level model, with
granular components/setters where useful. An animated custom LED strip must
participate in the existing emission/bloom machinery without pretending to
be `TOON_MESH` or reproducing Toon-specific renderer branches.

This updates the 2026-09 authoring direction that reserved `Shader` for custom
shading and explicitly ruled out a public `Material` component. `Shading`
already exists and remains a convenient built-in surface-model selector; the
new material contract is broader. Constructor spelling below is exploratory.
There is no implemented custom material to rename during this docs pass.

## Existing documents and what they cover

| Document | Reusable work / adjustment needed |
| --- | --- |
| [Materials v2](materials-v2.md) | Definitions vs instances, automatic vertex-family selection, typed schemas, pipeline keys. Retain these internals; replace its latest `Shading`/`Shader`-only public direction with the material anchor. |
| [Shader component draft](../../draft/shader-component.md) | Shared source identity, custom fragment loading, interface validation, live parameters, fallback. Treat `Shader` component syntax as an earlier proposal; program identity belongs inside the material definition. |
| [Custom fragment first slice](../mms-custom-fragment-shader-first-slice.md) | A deliberately narrow opaque/f32 proof, asynchronous compilation, bounded uploads, and counters. Reuse its gates under `Material`; emission is a subsequent explicit slice rather than assuming one color output supplies bloom. |
| [Animated material inputs](../animated-shader-material-inputs-mms-animation-system.md) | Input ownership, animation, update frequency, and cost investigation. Decide literal/setter/global/provider behavior as one material input contract. |
| [Shading cascade](../shading-model-components-and-cascade.md) | Local vs inherited declaration, complete-model replacement, source-linked GLTF projection. Reconcile with full material declarations and granular property overrides. |
| [Material descriptor caching](../material-descriptor-cache-update-frequency.md) | Frequent parameter updates must not allocate persistent UBOs/descriptors per distinct value. Reuse measurements and frame-safe storage work. |
| [Material renderer-resource graph](../material-renderer-resource-graph.md) | Pass/resource dependencies, view multiplicity, state, and cost inventory. Document capability activation independently of material names. |
| [Transmission epic](transmissive-materials.md) and [refraction spec](../../spec/material/refraction.md) | Specialized phase/resources and validation. These consume the general contract; custom fragment files do not automatically gain scene-color access or transmission support. |

We have substantial custom-shading and internal material design, but no complete
public custom-material contract joining these responsibilities. This epic is
that integration plan, not a second competing program registry.

## Checked implementation boundary

### 2026-10-23 acceptance and terminology

The user reports that the updated material-instance test scene works. Both VR
checkpoints are accepted. The selected model has exactly three layers:
**MaterialBase -> MaterialDefinition -> Material**. MaterialBase renames the earlier
MaterialFamily concept; it is not an additional layer. The later MMS discussion
selected Material for the instance (currently `MaterialInstance` in Rust),
superseding the earlier Material-as-definition naming. Source names are
unchanged in this documentation pass.

The registry/identity/revision/binding foundation is implemented, but the data
and integration remain specialized: instance parameters and definition defaults
are `AnimeShadingParams`, schemas are fixed to None/AnimeV1, and Shading-specific
resolution, fixed UBO packing and built-in program/state branches remain.
[The accepted checkpoint](../material-instances-anime-vr-checkpoint.md) records
these boundaries. Generalizing them is still required before the custom-f32
program slice; the full generic material system is not complete.

### 2026-10-23 stable instance foundation

[The second VR checkpoint](../material-instances-anime-vr-checkpoint.md) keeps
the same scene and adds generation-checked family/definition/instance records,
retained Anime/Toon values, material IDs in draw batches and bounded reusable
GPU UBO/descriptor snapshots. Scalar edits leave identity/batching unchanged;
GLTF projections consume the authored source. A shared-pulse control exercises
continuous updates. Automated GPU tests cover 10,000 edits, retained recordings
and independent stereo output. The user has now accepted this checkpoint in VR.
Custom programs, LED/burning shaders, broad serialization and generalized
emission classification remain pending. The previous sections below record
earlier checkpoints rather than current missing-instance status.

### 2026-10-22 first VR checkpoint

Shared leaf Anime/Toon Shading declarations now support multiple attachments,
detach-one/all and shared-source lifetime handling. The existing single-parent
automated checks are accepted as the baseline; manual car validation no longer
blocks this slice. [The VR scene and checklist](../shared-materials-xr-checkpoint.md)
exercise one source on two primitives plus an independent comparison. After
correcting the avatar-free controller topology, the user confirmed independent
rightmost-sphere shading, detaching the source from the other objects and
reattaching both. At that checkpoint, source implementation was paused and the
renderer still copied Anime values into renderables. The 2026-10-23 checkpoint
above supersedes that implementation boundary; the broader multi-parent task
remains incomplete.

### 2026-10-14 progress audit

Git history identifies this epic and its prerequisite tasks as the current
**Materials v3** plan: `dce84efc` introduced the epic on 2026-10-10;
`bac0d61f` updated the v3 proposal that day. Source inspection on 2026-10-14
still finds the large refactor pending; the existing built-in renderer is the
starting point, not a completed generic material-instance implementation.

| Slice | Checked status |
| --- | --- |
| Built-in groundwork | Anime/Toon shading, fixed material handles, and bounded immutable descriptor caching exist. These precede the v3 refactor. |
| [Stable instances and family contracts](../material-instances-and-family-contracts.md) | Planned; no generic family/definition/instance registry. Anime remains the first internal prototype. |
| [LED example preparation](../custom-materials-led-strip-example-preparation.md) | Planned; `examples/custom-materials.mms` is absent. Existing strip assets remain the preparation starting point. |
| [Opaque custom program with animated f32 input](../material-f32-time-animation-first-slice.md) | Planned; no MMS `Material.custom` / `input_f32` API or generic custom surface-program loader. |
| [Material component and resolved contract](../material-component-and-resolved-contract.md) | Planned; authored properties still resolve through existing specialized paths. |
| [Emission capabilities and LED proof](../material-emission-capabilities-and-led-strip.md) | Planned; extraction still recognizes fixed emissive Toon handles. |
| [Lighting/emission/extraction policy](../material-lighting-emission-and-extraction-policy.md) | Proposed on 2026-10-13; zero-intensity unlit behavior is not implemented. |
| [Water and refracted lighting](../water-surface-and-refracted-lighting.md) | Later consumer; existing transmission resources do not implement the proposed combined water surface or receiver lighting. |

The next material implementation slice remains the minimal stable-instance
foundation using Anime, followed by the opaque custom-program/f32-time proof.
This audit is read-only source inspection, not runtime renderer validation.

### Implementation at the 2026-10-14 audit

- [Material and MaterialHandle](../../../src/engine/graphics/primitives.rs)
  currently name fixed shader paths and built-in handles. The existing Rust
  `Material` struct is not an MMS `MaterialComponent` or a dynamic definition/
  instance registry.
- [ShadingComponent](../../../src/engine/ecs/component/anime_shading.rs)
  implements Anime and Toon, with an `AnimeShadingComponent` compatibility alias.
  [RenderableSystem](../../../src/engine/ecs/system/renderable_system.rs)
  resolves source scope and updates source-linked GLTF projections. Generic
  custom shader loading/schemas are still proposed.
- Color, texture/filtering, emissive intensity, cutout, background, and other
  style/phase inputs are distributed across components and renderable state.
  They need one documented resolved contract, not necessarily one giant ECS
  struct or immediate deletion of all existing components.
- `RenderableSystem::material_with_emissive` switches Toon handles to/from
  emissive Toon variants. [VisualWorld](../../../src/engine/graphics/visual_world.rs)
  recognizes emission by those handles. [vulkano_cbb](../../../src/engine/graphics/vulkano_cbb.rs)
  likewise selects fixed emissive pipeline variants.
- [The emissive fragment](../../../assets/shaders/emissive-toon-mesh.frag)
  currently extracts textured/vertex color multiplied by instance emissive
  intensity. A custom strip pattern would not appear in that extraction shader
  merely because its main-color fragment uses the same geometry.

These are specific coupling points to audit; this epic does not claim that all
renderer variants or post-processing behavior were validated in this docs pass.

## MaterialBase, MaterialDefinition, and Material

First internal prerequisite: [stable material instances and base contracts](../material-instances-and-family-contracts.md).
Anime provides the built-in prototype. Introduce stable generation-safe handles,
indexed draw bindings, and bounded mutable storage before loading custom programs.
Bases declare standard instance-input support and renderer contracts;
definitions select programs and additional inputs. Consuming emissive intensity
and opting into extraction/Bloom are separate declarations.

Keep exactly three authored material identities separate. MaterialBase replaces
the earlier MaterialFamily name; MaterialDefinition is the middle layer and
Material is the concrete scene-facing instance:

| Record | Responsibility |
| --- | --- |
| `MaterialBase` | Renderer interfaces, supported instance inputs/features, renderer state/configuration contract and required resources; current Rust name is `MaterialFamily` |
| `MaterialDefinition` | Shader/program selection, parameter types/layout and supported choices within the base contract; current Rust record name is `MaterialDefinition` |
| `Material` | Stable shared authored identity, initial/current typed values and resource bindings, explicit updates and generation/dirty state; current Rust record name is `MaterialInstance` |

`ResolvedMaterial` / the draw contract is a derived resolution result, not a
fourth authored layer. It records the selected material/instance plus resolved
color/texture/emission/alpha and provenance, geometry/view compatibility, pass
participation, batching and pipeline keys.

The accepted built-in implementation provides initial records under the source
names above. Generic values, custom programs and full base-driven state/feature
selection remain planned. Renderer globals
and per-view resources remain renderer-owned. Vulkan pipelines/descriptors are
cached renderer products, not mutable MMS fields. Dynamic scalar changes dirty
uploads; resource changes dirty bindings; changing program/state/capability may
require pipeline/pass/batch invalidation. The schema records that distinction.

Resolve an automatic static/cached-deformed vertex family from actual geometry,
as Materials v2 proposes. Keep explicit custom vertex selection as a later
validated extension. Start with named supported state profiles rather than
unrestricted Vulkan state or arbitrary user-declared render passes.

## High-level authoring and granular controls

Use `MaterialDefinition(base)` to author a reusable shader/schema and
`Material(definition)` to construct independent instances. `Material(base)`
creates an instance with an implicit definition; `MaterialBase()` selects the
default surface without extraction or transmission. Without customization,
use its standard default shading implementation. `.fragment(...)` configures
the implicit definition, while `.descriptor_set(...)` on Material supplies
initial values validated against the shader's declared types. Definition
schemas instead use `field: Type`. Whether typed records with values may supply
either role through explicit type/data views is still open. See the [authoring contract](../material-component-and-resolved-contract.md#authoring-decision-2026-10-23)
for sharing, initial values and open descriptor-layout details.
`Shading.anime()`/`.toon()` configure
the surface model in that same resolved contract. Existing color, texture,
emissive, alpha/cutout and other components remain useful granular overrides
when supported by the selected definition.

`Material` supports both attachment forms already used by `Shading`:

- A Material wrapper is a scope for its descendant renderables, including
  descendants behind ordinary transforms and generated combined mesh outputs.
- A Material directly beneath a renderable declares that renderable's local
  material, overriding an inherited wrapper. A declaration directly beneath
  a container scopes its descendants, following the shading-style convention.

Illustrative Phase 1 forms, with constructor spelling still proposed:

```mms
Material(MaterialBase())
    .fragment("assets/shaders/animated-led-strip.frag")
    .descriptor_set({ time_seconds = 0.0 }) {
    CombineMesh {
        // All LED rectangles resolve this same material instance.
        R.cube() {}
        R.cube() {}
    }
}

R.cube() {
    Material(MaterialBase())
        .fragment("assets/shaders/animated-led-strip.frag")
        .descriptor_set({ time_seconds = 0.0 })
}
```

The wrapper owns one live material instance; inheriting it does not instantiate
one material per LED. Resolve the first CombineMesh source's **effective**
material, including wrapper inheritance, and transfer its definition/instance
identity to the generated primitive. For this asset all source LEDs resolve
the same instance. Keep the wrapper alive outside the collapsed geometry so
time updates and late shader readiness continue reaching the combined output.
Copying the source's default Toon handle or a frozen parameter snapshot would
not satisfy this rule. Different source materials are not combined into one
multi-material primitive by this slice.

One instantiated Material component means one logical renderer material
instance; another instantiation means another instance even with identical
values. Structural components still reparent, while leaf Shading declarations
now support simultaneous attachments through the
[multi-parent component refactor](../multi-parent-components-and-material-sharing.md).
The shared Anime case is user-confirmed in VR. The public Material component
and its generic renderer instance bridge remain pending; do not infer an
implemented generic Material API from that proof. The second checkpoint does
provide the built-in renderer instance registry under existing Rust names.
Reference bindings and generated projections must share the source instance
rather than create additional Material instances when that bridge is introduced.

The resolver must define full-material replacement vs individual property
inheritance. Proposed starting policy:

1. A local complete material replaces the inherited definition and its defaults.
2. Granular color/texture/emission/alpha overrides resolve independently with
   explicit provenance. Validate them against that definition's capabilities.
3. A `Shading` declaration is a built-in surface selection, not an automatic
   extra program layered on a custom fragment. A custom material and explicit
   built-in shading selection at the same scope conflict unless a future
   documented composition constructor authorizes them.
4. Two competing complete material declarations at one scope are errors; child
   order is not precedence. Reparent/removal and GLTF regeneration re-resolve
   consumers while preserving the authored shared instance identity.

Confirm this with a compatibility table before implementation: existing shading
wrappers and model-local parameters must not silently disappear. Define how
inherited granular settings cross a full-material boundary; record resets
explicitly rather than mixing arbitrary fields from incompatible models.

Inheriting a built-in *preset* may copy supported defaults/interface choices.
It does not grant render capabilities by inheriting a Toon handle, and does
not run a Toon fragment alongside the custom fragment. Shader-function/library
reuse is separate from material/state inheritance.

## LED strip and input update contract

First establish [Phase 1: f32-only time animation](../material-f32-time-animation-first-slice.md)
on opaque static geometry. The MMS author chooses a different field-name set
and count for each definition, with all custom fields f32. An ordinary f32
declaration builder or names-only list establishes the retained schema; shader
reflection validates its names, types and GPU layout. Type expressions and the
record-role design gate are not prerequisites for this narrow API. Full source
annotations/static checking are separate language work. Explicit initial values/setter updates
are sufficient; callbacks stored
as material inputs and automatic renderer/global bindings are not first-phase
prerequisites. The LED example
below adds renderer emission in a subsequent slice.

Desired shape, **not valid committed MMS syntax**:

```mms
import { f32 } from "types"

// extraction_base is an engine-provided extraction-capable MaterialBase;
// its lookup spelling and extraction program contract remain to be specified.
let light_strip_definition = MaterialDefinition(extraction_base)
    .fragment("assets/shaders/led_light_strip.frag")
    .descriptor_set({ glow_amount: f32 })

let light_strip_material = Material(light_strip_definition)
    .descriptor_set({ glow_amount = 0.0 })
```

An illustrative explicit update path:

```mms
on_global("FrameTick", fn(event) {
    light_strip_material.set_input("glow_amount", get_current_glow_amount())
})
```

The function and material methods here are proposed. Avoid requiring typed MMS
annotations or arbitrary callbacks merely to prove one float upload. Runtime
type identities and schema type expressions are required; primitive tag values
alone do not implement those records. Settle the record-role design first,
then freeze remaining descriptor-table/layout details in the authoring ticket.

Distinguish four input sources: literal defaults, explicit retained setters/
animation channels, renderer-owned globals, and optional script/provider
bindings. Never invoke MMS closures from draw recording, shader compilation,
or the GPU. A later `fn()` binding would evaluate at a documented engine/script
boundary, once per shared material/input update rather than once per draw/eye.
Specify error/last-valid-value behavior and dependency/lifetime handling before
adding it. Render-to-texture and stereo views read a coherent retained revision.

`transport_beat` should mean an actual transport beat input; `glow_amount`
should mean the computed glow. Do not conflate renderer time, audio transport
beat, or script callback time. Reuse the engine's transport read/update contract
when connecting beat animation, and reset/scrub intentionally. Initial support
is a finite `f32` plus explicit setter; richer types/providers follow schemas
and measured use cases. Shared material updates affect all consumers; independent
strip animation requires explicitly separate instances/overrides.

## Emission as a renderer capability

Policy review: [lighting, visible emission, and extraction](../material-lighting-emission-and-extraction-policy.md)
specifies the proposed zero-intensity behavior: an emissive/unlit surface
keeps its base color and skips lighting and extraction at 0. Lighting selection
must remain independent of intensity and global extraction state. Decide
whether positive visible emission also needs an authored extraction opt-out.
This review feeds the emission slice; it does not block the opaque f32 proof.

Separate visible emission, emission radiance/intensity, eligibility for emission
extraction, and bloom enable/configuration. An extraction-capable MaterialBase
declares the supported capability; it is not sufficient shader output by itself
and does not turn bloom on globally. `Emissive` sets supported instance/material
intensity; `EmissivePass`/Bloom remain renderer/post-process configuration.

The custom material must provide emission consistent with its animated pattern
and alpha/coverage in both main-color and extraction rendering. Evaluate a
shared surface/emission shader contract with generated pass variants against
an explicit emission fragment entry/program using the same typed inputs. Choose
one for the first slice and verify output attachments/interfaces; do not reuse
the fixed Toon extraction shader for an arbitrary fragment or assume opaque
`MeshSurfaceV1`'s single color output already includes an emission attachment.

Pipeline resolution and draw-cache classification use declared capabilities
and supported phase variants rather than equality with emissive Toon handles.
Audit depth/occlusion, cutout, clipping/stencil, transparent ordering, background
eligibility, cached-deformed geometry, and view-specific resources. Unsupported
combinations need actionable validation and a documented fallback. Emission
must not silently authorize extra passes on unsupported geometry.

Preserve existing post-process order and ownership. In particular the known
[refraction/Bloom depth-ordering issue](../../bugs/refraction-behind-emissive-bloom-depth-ordering.md)
is a separate visibility-policy question; extracting the capability does not
automatically solve halo ownership. Record supported LED phases and keep
unused emission work/resources conditional.

## Delivery and 0.10.0 completion gates

- [ ] [Multi-parent component sharing](../multi-parent-components-and-material-sharing.md):
  carefully migrate graph mutation, lifecycle, inheritance/queries and affected
  system callers; verify shared Shading/Material sources in unrelated locations
  and benchmark ordinary trees, shared fan-out, and diamond graphs.

- [ ] [Internal material instances and family contracts](../material-instances-and-family-contracts.md):
  use Anime to prove stable identities, bounded uploads, indexed draw bindings,
  and standard-instance-input/pass metadata before the custom-program slice.

- [ ] [Prepare the LED asset and custom-materials scene](../custom-materials-led-strip-example-preparation.md):
  exactly four strips around a plane in `examples/custom-materials.mms`, Toon
  plastic/backing separated from combined LED surfaces using `animated_led_strip`,
  an FPS camera, low ambient light, and the implicit-cloud background. Stage
  built-in preparation, f32 animation, then emission/Bloom in the same scene.

**First custom-program phase:** after the internal prerequisite, complete the [f32 time-animation slice](../material-f32-time-animation-first-slice.md)
with minimal Material ownership/resolution, one opaque custom fragment, and
explicit time updates. Do not require the full migration or emission paths
before this example can run. Subsequent release work is listed below.

1. [ ] [Material component and resolved contract](../material-component-and-resolved-contract.md):
   inventory existing controls, freeze data/ownership/cascade/update semantics,
   and add a compatibility resolver with built-in behavior preserved.
2. [ ] Adapt the [opaque custom-fragment proof](../mms-custom-fragment-shader-first-slice.md)
   through the current [f32 time-animation slice](../material-f32-time-animation-first-slice.md)
   to `Material`; validate f32-only inputs, asynchronous program loading, fallback,
   program/pipeline sharing, independent instances, and bounded frame-safe
   uploads. Carry forward its sustained-update and removal tests.
3. [ ] [Emission capabilities and LED proof](../material-emission-capabilities-and-led-strip.md):
   remove emissive Toon identity assumptions behind a compatibility bridge and
   prove the same animated pattern contributes to visible color and extraction.
4. [ ] Complete required [input/animation investigation](../animated-shader-material-inputs-mms-animation-system.md)
   and [descriptor policy](../material-descriptor-cache-update-frequency.md)
   for that scene. Record update cadence, frame coherence, CPU/GPU cost,
   compile/pipeline counts, uploads, and allocation high-water marks.
5. [ ] Update the [material/resource graph](../material-renderer-resource-graph.md)
   and public material docs with supported capabilities, formats/view multiplicity,
   phase ordering, MMS examples, serialization and safe resource retirement.

0.10.0 requires a working `Material` anchor, built-in compatibility, custom
fragment/scalar support, and a custom animated emissive LED strip with bloom
and desktop/XR proof. Validate ordinary static meshes first, then supported
cached-deformed/GLTF consumers under the same definition. Preserve existing
cutout/clipping and emission scenes through the resolver; only advertise custom
variants that have their own validated program/pass contract.

Arbitrary vertex/compute programs, unrestricted render passes/state, node editors,
all parameter/resource types, automatic closure bindings, broad hot reload, and
wholesale conversion of every special material are follow-ups. Full transmission
and mirror migrations retain their own tickets; this release foundation must
keep those existing behaviors valid without requiring their redesign.

Later capability consumer: [water surface optics and refracted lighting](../water-surface-and-refracted-lighting.md)
uses these definition/instance/family contracts for combined reflection,
refraction and specular shading, plus a separate light-transport/receiver path.
Water is not an additional requirement of the initial opaque f32 milestone or
the 0.10.0 completion gates.
