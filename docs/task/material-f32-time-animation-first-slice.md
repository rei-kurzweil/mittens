# Phase 1: custom material with f32 time-animation inputs

Date: 2026-10-10
Status: named-f32 room path implemented and headless-GPU tested 2026-10-24; manual VR acceptance and broader compatibility gates pending.

2026-10-25: symmetric `.inputs(...)` authoring is implemented. Definitions take
colon-style primitive type records; instances take equals-style values. The
room uses newline-separated fields without commas. `.input_f32(...)` and
`.descriptor_set(...)` remain compatibility entry points for the historical
examples below. The GPU path remains f32-only and UBO-backed; mixed records,
vector uploads and the full built-in/common-flow migration remain pending.
Parent: [custom materials epic](epic/custom-materials.md).
Release: [0.10.0](epic/0.10.0/README.md).

Current checkpoint: [youtuber room](youtuber-room-custom-material-checkpoint.md),
with the actual API, initialization path, limits, validation and remaining work.
This scene supersedes the previously proposed four-edge-plane fixture.

Internal prerequisite: [material instances and family contracts](material-instances-and-family-contracts.md).
First prove stable instance handles, standard input metadata, and bounded mutable
GPU bindings with Anime. This is the first **custom-program** proof, not a bypass
of that foundation. Newly authored inputs are f32-only; existing standard
instance color/opacity/emission data retains its own declared interface types.

2026-10-23 authoring revision: the
[MaterialBase / MaterialDefinition / Material contract](material-component-and-resolved-contract.md#authoring-decision-2026-10-23)
supersedes `Material.custom(...).input_f32(...)`. Land lowercase first-class
[MMS type tags](../../crates/meow-meow-script/docs/draft/type-system-epic.md#runtime-type-tag-direction-2026-10-14)
before definition schema tables. Those tags may include f32/vector identities
without implementing all corresponding GPU field types in this first proof.
The primitive tag-value/import/storage/transport prerequisite is implemented.
The subsequent [record-role revision](../../crates/meow-meow-script/docs/draft/record-types-and-values.md)
proposes colon-style schemas for the eventual generic API. The later scope
clarification selects a narrower first slice: MMS authors declare a variable
number of custom-named fields per MaterialDefinition, all restricted to f32.
The implemented declaration API is `.input_f32(name)` on MaterialDefinition.
It uses a real retained schema and GPU layout validation, but not
the pending `TypeExpression` AST or record-role semantics.
MaterialDefinition/Material registration, local-file fragment compilation,
reflection and bounded custom uploads are now implemented for opaque static meshes.
Full source annotations, numeric default initialization and precise numeric
`typeof` are separate MMS work, not required to pass explicit initial values.

## Goal

Prove a custom opaque fragment material on ordinary static meshes with named
**f32-only inputs**. Animate one `time_seconds` field through normal MMS updates
and show that the shader changes continuously without mesh rebuilds, shader
recompilation, pipeline creation, or growing persistent descriptor allocations.

This is the first custom-material implementation target. Build only the Material
component/definition/instance and resolver infrastructure needed for this proof;
the full cascade migration, extra geometry families, emission/Bloom, and richer
types are subsequent slices. Preserve existing built-in rendering while adding
the new path. Reuse the loading/validation/storage tests from the earlier
[custom-fragment task](mms-custom-fragment-shader-first-slice.md).

## Input and authoring contract

The MMS author declares the names and count of each definition's custom fields.
Every field has type f32 in this slice; instances supply finite initial values
and subsequent updates. Supporting different definitions with different names
and field counts is required, not an optional extension of a hard-coded
`time_seconds` field. A compiled definition's schema is fixed; changing a value
does not add, remove or rename fields.

Shader reflection checks the MMS-authored schema against the declared shader
interface and supplies/validates byte offsets, block size and binding layout.
It does not replace the author's field declaration with a fixed engine field
list or silently rename inputs. Fields may occupy a single material uniform
buffer binding; a distinct Vulkan descriptor per float is not required, and
compatible descriptor-set layouts may be shared.

The selected first-slice spelling is the definition-specific f32 builder below;
it does not require colon-style type expressions or resurrect the earlier
combined `Material.custom` definition/instance API.
Instances continue to supply ordinary equals-style value tables.

Implemented first-slice syntax (the room uses the same five fields):

```mms
let definition = MaterialDefinition(MaterialBase())
    .fragment("assets/shaders/mms-time-test.frag")
    .input_f32("time_seconds")
    .input_f32("speed")
    .input_f32("intensity")

let animated_material = Material(definition)
    .descriptor_set({ time_seconds = 0.0, speed = 1.0, intensity = 1.0 })

R.cube() { animated_material }
R.sphere() { animated_material }

let animation = { elapsed_seconds = 0.0 }
on_global("FrameTick", fn(event) {
    animation.elapsed_seconds = animation.elapsed_seconds + event.dt_sec
    animated_material.set_input("time_seconds", animation.elapsed_seconds)
})
```

`.input_f32(name)` declares a field on a definition; values belong to the Material
instance. Missing declared initial values default to zero. Registration is queued
when the expression finishes, independently of scene attachment.

`Material(base)` remains a proposed later shorthand, not implemented by the room
checkpoint; use `Material(definition)` today. For that future implicit
definition: `.fragment(...)` supplies its program and `.descriptor_set(...)`
supplies values whose declared types come from the shader interface/reflection.
Exact descriptor table/binding structure and getter/setter spelling remain
proposed; the required contract is named f32 declarations plus retained updates.
Validate numeric-to-f32 conversion, finiteness after conversion, names/defaults, stale handles, and
shader field compatibility. Reject unsupported authored types explicitly.
Time values have no implicit `[0, 1]` clamp; units are declared by field name/
documentation. A separate `phase` or `glow_amount` can be a finite scalar too.

The MMS handler computes and writes a value. The material input is not a
stored `fn()` evaluated during rendering. Only the script/main-thread update
path invokes that handler; every consumer/view reads the same retained material
revision. A shared material source shares its value; separately constructed
instances can animate independently while sharing program/pipeline resources.

`time_seconds` in this example means elapsed FrameTick time since scene setup.
It does not promise wall-clock precision, renderer-global time, or audio
transport phase. Start here; an explicit `transport_beat` f32 can follow once
the transport read/pause/seek contract is established. Bound GPU precision for
long-running animations using an explicitly chosen elapsed/phase strategy;
do not silently wrap time and break arbitrary shaders. A global time binding
or Animation/Keyframe channel is a later input-source feature, not required to
upload an f32 from an existing callback.

## Renderer and validation scope

- Ordinary opaque static meshes, the engine vertex stage, and the validated
  versioned `MeshSurfaceV1` interface only. No authored vertex programs.
- Async custom fragment loading/compilation/reflection, useful diagnostics,
  built-in fallback while pending/failed, and cached compatible pipelines.
- Match each input name to a shader material-block float. Validate layout,
  varyings, descriptors, and target state; MMS never packs GPU bytes itself.
- Stable material instance identity and dirty-tracked, bounded frames-in-flight
  UBO/descriptor slots. Values do not enter program/pipeline cache identity.
- Custom vectors/colors, matrices, integers/booleans, arrays/structs,
  textures/samplers, per-instance attributes, and raw buffers are excluded.
  Existing engine-owned descriptors and built-in inputs can still be used
  according to the fixed interface.
- The room follow-up supports extraction of the same custom radiance output on
  opaque foreground meshes through `MaterialBase.extracted_emissive()`.
  Transparency/cutout, custom clipping variants, GLTF/
  cached-deformed support, hot reload, and complete material serialization/
  migration remain later acceptance slices.

## Proof and exit gate

Use [examples/youtuber-room.mms](../../examples/youtuber-room.mms), selected by
the user on 2026-10-24. Following the initial quad proof, 308 LED cubes now bake
into four CombineMesh outputs sharing one material and perimeter UV coordinates.
The visible wave has one CPU time update per frame. White backings and furniture
use built-in Anime shading. The extracted-emissive base feeds the same animated
radiance into visible color and Bloom. Independent instance values are checked
in MMS tests; one-field and five-field definitions are checked on the GPU.

The earlier [LED-strip asset preparation](custom-materials-led-strip-example-preparation.md)
remains a reusable-asset follow-up. The room and focused tests now cover custom
material retention and authored UV transfer through CombineMesh source collapse.

- [x] Construction and retained f32 get/set work in top-level and runtime MMS.
- [x] MMS-authored definitions declare different custom names and field counts,
  with every declared field constrained to f32. Verify one-field and multi-field
  definitions and reject duplicates or incompatible shader declarations. No
  TypeExpression syntax is required for this first slice.
- [x] Material records accept finite numeric initial values and updates; reject
  unknown names and type tags where ordinary values are required. Verify explicit
  definition construction; the implicit-definition shorthand remains later work. Keep
  declared field names with mutable instance values.
- [ ] Invalid/nonfinite/out-of-range conversion, unknown fields, incompatible
  shader layouts, and failed compilation produce useful errors/fallback.
- [ ] Shared consumers stay synchronized; independent instance values stay
  independent. Compatible instances share one compilation/pipeline.
- [x] At least 10,000 distinct finite updates cause uploads but no further
  shader compilation/pipeline creation and no persistent UBO/descriptor growth
  after documented warm-up. Record compile/pipeline/upload/allocation counters.
- [ ] Removing source/consumers safely retires resources while frames are in
  flight and leaves the remaining shared-program instances rendering.
- [ ] Record the runnable example, visible animation, measured update costs,
  and bounded storage before marking Phase 1 complete.

Fuller MMS types should later supply value type identities/checking rather than
introduce a competing material type system. Material-specific shader layout,
ownership, range, and update metadata still remains necessary. That integration
is not a dependency of this f32-only phase.
