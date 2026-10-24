# Stable Anime material instances: second VR checkpoint

Date: 2026-10-23
Status: built-in instance foundation accepted in VR by the user; generic parameter/resolution migration remains pending.
Parent: [instance/base foundation](material-instances-and-family-contracts.md).
Previous acceptance: [shared Shading VR checkpoint](shared-materials-xr-checkpoint.md).

Later implementation: [2026-10-24 custom-f32 room checkpoint](youtuber-room-custom-material-checkpoint.md).
The inventory below records the accepted built-in proof's boundary; custom
definition registration/reflection/upload has since been added. Built-in and
custom lifecycle/resolution unification remains a follow-up, not a completed
part of that newer proof.

## Accepted result and naming decision

The user retested the updated scene and reported that everything seems to work.
Record the second VR checkpoint as accepted. Allocation bounds and retained
revision guarantees still come from the automated tests below; the headset
report does not supply a measured test duration or allocation trace.

The selected terminology has exactly three layers:

| Concept | Current Rust name | Responsibility |
| --- | --- | --- |
| `MaterialBase` | `MaterialFamily` / `MaterialFamilyId` | Renderer interfaces, supported features, render-state/configuration contract |
| `MaterialDefinition` | `MaterialDefinition` / `MaterialDefinitionId` | Shader/program choice and typed input schema within that contract |
| `Material` (the instance) | `MaterialInstance` / `MaterialInstanceId` | Stable shared identity and independently mutable values/bindings for one definition |

`MaterialBase` replaces the earlier name `MaterialFamily`; they are the same
concept, not two layers. The later 2026-10-23 MMS API discussion selected
`Material` for the concrete instance, superseding the earlier Material-as-
definition naming. `MaterialDefinition(base)` declares field types;
`Material(definition)` supplies initial values. `Material(base)` creates an
instance with an implicit definition; `MaterialBase()` is the default surface
without extraction or transmission. See the
[authoring contract](material-component-and-resolved-contract.md#authoring-decision-2026-10-23)
for the proposed type-tag tables and constructor overloads. The current Rust
names are recorded for accuracy; this
documentation pass does not rename source symbols or add an abstraction.
Renderer state/capability metadata is currently split between the family and
definition records and still needs migration toward that base contract.

The older `primitives::Material` shader-path pair is compatibility code to fold
into the middle layer, not an additional conceptual material layer. A drawable's
renderable/GPU instance and resolved draw state are consumers/results of this
model, not additional authored material layers.

## What is generic and what is still specialized

The ID/ownership/revision path is implemented: registry records, draw bindings,
reverse consumer tracking and bounded GPU snapshots use material-instance
identity. This is a working built-in foundation, not a completed generic
material authoring/parameter system.

Remaining specialization checked in source:

- `MaterialInstance.parameters` and `MaterialDefinition.defaults` are still
  `AnimeShadingParams`, even for the Toon compatibility record. The available
  schema enum contains only `None` and `AnimeV1`.
- VisualWorld still defines that Anime parameter type and exposes Anime-specific
  read helpers. World has Shading-specific shareability and topology-dirty
  bookkeeping; it does not own a separate Anime uniform-value registry.
- RenderableSystem still uses Shading/Anime-specific registration, resolution
  and pending-binding hooks to connect authored components to registry IDs.
- Vulkan upload packing still builds the fixed built-in `MaterialUBO`, including
  Anime fields. Program/state selection and emission retain built-in handle
  branches rather than being fully driven by the base/material contract.

Next work, when authorized: generalize retained values/defaults and GPU layouts
through material schemas, then migrate the resolution/authoring bridge while
keeping Anime compatible. This prepares the opaque custom-f32 program proof.
No source changes are made while recording this acceptance and naming decision.

## Run the same scene

```sh
./target/release/mittens-engine load examples/shared-materials-xr.mms
```

The scene still has the shared cube/middle sphere, the independent rightmost
sphere, and the avatar-free controller pointers accepted in the first test.
Repeat the slider and detach/restore checks. They should behave as before.

The new **Start shared pulse** button continuously edits the shared material's
shade strength through one FrameTick setter per frame. Both shared objects
should pulse together; the rightmost sphere stays unchanged. Detach the middle
sphere while pulsing: it uses Toon and the cube keeps pulsing. Restore both:
they immediately share the current pulse. Detach both, leave the source pulsing,
and restore again to check that its identity/lifetime survives having no consumers.
Stop the pulse before editing sliders manually. Stopping synchronizes the shade
slider/readout to the retained value; panel restoration also reads current values.

Optional terminal allocation diagnostics:

```sh
CAT_DEBUG_MATERIAL_INSTANCES=1 ./target/release/mittens-engine load examples/shared-materials-xr.mms
```

`[MaterialInstances]` periodically prints live renderer material records,
cumulative UBO/descriptor allocations, uploads and snapshot reuses. With an
unchanged scene, allocations should settle after warm-up while pulse uploads
continue. Counts include other authored sources such as the floor's Toon source;
they are not counts of the three comparison objects or draw calls.

## Implemented representation

- `MaterialFamilyId`, `MaterialDefinitionId`, and `MaterialInstanceId` are the
  current source names for the base/definition/instance IDs and use
  generation-checked arenas. The first base describes the versioned fixed
  mesh-instance interface. Anime and Toon have separate shared definitions,
  schemas/defaults, compatibility-state profiles and input/capability metadata.
- One initialized authored Shading source owns one retained instance record.
  Equal-valued sources remain separate; shared attachments and GLTF projections
  consume the source ID. Runtime projection getters/setters address the source.
- Renderables and draw batches store the material-instance ID rather than
  copied Anime values. Scalar setters change the retained values/revision,
  leaving IDs and batch classification unchanged. Per-renderable color,
  opacity and emissive intensity retain their existing instance-data ownership.
- GPU material records resolve through an indexed table. Structural resource
  variants still use a bounded cache keyed by program/texture/filtering/quantization;
  changing shared Anime values does not create a value-keyed cache entry.
  Legacy per-renderable light quantization remains in that compatibility key;
  this slice does not optimize animations of that separate control.
- Each resource variant has at most eight UBO/descriptor snapshots. A matching
  revision can be shared by passes/eyes. An older snapshot is writable only
  when the cache is its descriptor's sole Arc owner; Vulkano buffer-write locks
  additionally check GPU use. Recorded-but-unsubmitted commands retain their
  original data. If all eight snapshots are retained, recording returns an
  explicit error rather than overwriting them or allocating without a bound.
- The structural variant cache retains at most 16 variants per source. This
  preserves current per-renderable texture/filtering/quantization behavior;
  broadly varying resource bindings are not optimized by the scalar-update proof.
- Runtime source destruction invalidates its ID and retires cache ownership;
  recorded commands retain their resources. Detachment alone preserves the
  source. Clearing VisualWorld preserves arena generation history while
  invalidating old instance IDs.
- A reverse consumer index targets source retirement and structural definition
  changes, including GLTF-generated consumers outside the source's ECS subtree.
  Scalar changes only update the shared record; they do not traverse consumers.

## Scope and validation

The registry and GPU path cover authored Anime/Toon sources and their existing
static/cached-deformed programs. Unauthored built-in fallback draws retain the
immutable descriptor cache. This is not the custom-program loader or MMS
`Material` / `MaterialDefinition` API. Custom f32 schemas/reflection, custom LED/burning programs,
generalized emission extraction, CombineMesh source transfer and broad material
serialization remain subsequent work. Existing emission classification still
uses the built-in compatibility path; metadata does not change pass activation.

Automated checks include 10,000 source edits with stable IDs/counts and no batch
rebuilds, independent equal-valued sources, shared colors/emissive instance data,
source destruction/reuse, GLTF projection identity and real scene callbacks for
the pulse and detach/restore controls.

Headless Vulkan checks use real UBOs/descriptors and stereo pixel readback:

- Hold all eight revisions, verify that another upload applies bounded
  backpressure, and verify that every retained revision is unchanged. Release
  them and perform 10,000 edits without another UBO/descriptor allocation.
- Render independently mutable Anime sources in both eyes over 12 frames.
  Two UBO/descriptor bindings are reused; no animated entry enters the legacy
  immutable cache.
- Record two different revisions before submitting either command buffer.
  Both retain their expected pixels; one additional snapshot protects the older
  recording. Existing stereo rendering with MSAA off/4x is also exercised.

Both VR checkpoints are now accepted by the user. Implementation stops here;
generic parameter/resolution work and custom programs remain separate next slices.

Focused validation: 68 CPU/runtime compatibility tests pass, and three explicit
headless Vulkan tests pass on a GeForce GTX 1080. These include static/skinned
GLTF source linking, projection getters/setters, structural definition changes,
source retirement and the actual pulse/slider/detach scene callbacks. The GPU
checks include independent stereo readback, retained unsubmitted recordings,
10,000 uploads without further allocations after warm-up, and the existing
stereo MSAA off/4x regression. The library also checks without desktop features.
