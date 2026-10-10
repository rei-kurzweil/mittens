# Epic: custom materials, typed inputs, and renderer capabilities

Date: 2026-10-09
Status: proposed architecture; documentation only.
Release: [mittens-engine 0.10.0](0.10.0/README.md).

2026-10-10 first-phase decision: [f32 time-animation proof](../material-f32-time-animation-first-slice.md)
is the first testable implementation slice. Authored shader inputs support
only finite `f32` values initially. Broader MMS type integration, additional
input types/providers, and renderer emission follow after that proof.

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

## Definition, instance, and resolved draw contract

Keep three identities separate:

| Record | Responsibility |
| --- | --- |
| `MaterialDefinition` | Built-in/custom surface program, versioned interfaces, parameter schema, supported vertex families, constrained render-state profile, declared renderer capabilities and required resources |
| `MaterialInstance` | Stable shared authored identity, current typed values/resource bindings, explicit update bindings, generation/dirty state, optional instanced copy policy |
| `ResolvedMaterial` / draw contract | Selected definition/instance plus resolved color/texture/emission/alpha and property provenance, geometry/view compatibility, pass participation, batch identity and pipeline key |

These are proposed data structures, not existing Rust API. Renderer globals
and per-view resources remain renderer-owned. Vulkan pipelines/descriptors are
cached renderer products, not mutable MMS fields. Dynamic scalar changes dirty
uploads; resource changes dirty bindings; changing program/state/capability may
require pipeline/pass/batch invalidation. The schema records that distinction.

Resolve an automatic static/cached-deformed vertex family from actual geometry,
as Materials v2 proposes. Keep explicit custom vertex selection as a later
validated extension. Start with named supported state profiles rather than
unrestricted Vulkan state or arbitrary user-declared render passes.

## High-level authoring and granular controls

Use `Material.custom(...)` for a full custom material, and a built-in material
constructor/selection path for built-ins. `Shading.anime()`/`.toon()` configure
the surface model in that same resolved contract. Existing color, texture,
emissive, alpha/cutout and other components remain useful granular overrides
when supported by the selected definition.

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
on opaque static geometry. Explicit default/setter updates are sufficient;
callbacks stored as material inputs, generic type declarations, and automatic
renderer/global bindings are not first-phase prerequisites. The LED example
below adds renderer emission in a subsequent slice.

Desired shape, **not valid committed MMS syntax**:

```mms
let light_strip_material = Material.custom({
    emissive = true
    fragment_stage = "assets/shaders/led_light_strip.frag"
    inputs = {
        glow_amount = 0.0  // schema declares f32 and validation
    }
})
```

An illustrative explicit update path:

```mms
on_global("FrameTick", fn(event) {
    light_strip_material.set_input("glow_amount", get_current_glow_amount())
})
```

The function and material methods here are proposed. Avoid requiring typed MMS
field declarations or arbitrary callbacks merely to prove one float upload.
Freeze actual constructor/schema syntax in the foundation ticket.

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

Separate visible emission, emission radiance/intensity, eligibility for emission
extraction, and bloom enable/configuration. `emissive = true` is shorthand for
a declared supported capability; it is not sufficient shader output by itself
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

- [ ] [Prepare the LED asset and custom-materials scene](../custom-materials-led-strip-example-preparation.md):
  exactly four strips around a plane in `examples/custom-materials.mms`, Toon
  plastic/backing separated from combined LED surfaces using `animated_led_strip`,
  an FPS camera, low ambient light, and the implicit-cloud background. Stage
  built-in preparation, f32 animation, then emission/Bloom in the same scene.

**First testable phase:** complete the [f32 time-animation slice](../material-f32-time-animation-first-slice.md)
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
