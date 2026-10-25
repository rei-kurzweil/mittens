# Task: Material component and resolved material contract

Date: 2026-10-09
Status: symmetric typed-f32 inputs API implemented 2026-10-25; full generic contract below remains proposed.
Parent: [custom materials epic](epic/custom-materials.md).
Release: [0.10.0](epic/0.10.0/README.md).

Implemented subset: [youtuber-room checkpoint](youtuber-room-custom-material-checkpoint.md).
`MaterialBase()`, `MaterialDefinition(base).fragment(path).inputs(schema)` and
`Material(definition).inputs(values)` register directly from MMS without
requiring scene attachment. The default opaque mesh base and
`MaterialBase.extracted_emissive()`, local GLSL files and named scalar-f32 fields
are supported. Colon-style primitive schemas are implemented; `Material(base)`
implicit-definition convenience and vector GPU fields remain future API. Built-in definitions now share the [common material flow](material-instances-and-family-contracts.md#first-refactoring-slices-and-gates),
regardless of whether they remain authored in Rust/GLSL.

2026-10-25 input API review: the implemented common authoring method is
`.inputs(...)` on both MaterialDefinition and Material: field types on the
definition, corresponding field values on the instance. This supersedes
`.descriptor_set(...)` in the generic API below. The older
`.input_f32(...)` / `.descriptor_set(...)` entry points remain compatibility APIs.
The room uses the symmetric spelling, with f32 type fields and value expressions.

Internal prerequisite: [stable instances and family contracts](material-instances-and-family-contracts.md),
proved using Anime before custom programs. First custom-material target:
[f32-only time-animation slice](material-f32-time-animation-first-slice.md).
Implement the minimum anchor/resolution needed for its opaque static example
before expanding to this task's full compatibility/cascade contract. The wider
inventory can proceed independently; it must not turn Phase 1 into a wholesale
material migration.

2026-10-23 primitive-tag prerequisite completed: MMS now imports primitive type-tag values
from `"types"` (`f32`, `vec2`, `vec3`, `vec4`, `str`) and preserves them in
schema tables, closures and typed host/callback transport. Material components,
descriptor tables, reflection/upload integration and constructors below remain
proposed. `typeof`, annotations/default numeric initialization and `mat4` are
separate pending work. See the [implemented language boundary](../../crates/meow-meow-script/docs/draft/type-system-epic.md#implemented-tag-value-boundary-2026-10-23).

The subsequent [record/type/value proposal](../../crates/meow-meow-script/docs/draft/record-types-and-values.md)
revises definition schemas to colon-style type expressions. For both type and
value tables, commas are optional between newline-separated fields; same-line
fields require commas. Trailing commas are allowed.
The primitive-record `TypeExpression` AST/resolution subset is now implemented;
mixed/typed-value records remain pending. The earlier first-slice decision allowed a current-grammar
f32 declaration builder or names-only list on MaterialDefinition: each MMS author
chooses field names/count, every field is f32, and shader reflection validates
the schema/layout. This narrow API can precede type expressions and the open
record-role semantics. The colon-style examples below describe the eventual
generic API beyond the implemented primitive-f32 subset.

## Authoring decision (2026-10-23)

There are exactly three layers, with the following public MMS names. This
supersedes the earlier Material-as-definition / MaterialInstance authoring
proposal and the combined `Material.custom(...).input_f32(...)` examples.
Current Rust source names remain unchanged in this documentation pass.

| MMS type | Responsibility | Current Rust record |
| --- | --- | --- |
| `MaterialBase` | Read-only engine-provided renderer interfaces, features, state and resource contract | `MaterialBase` |
| `MaterialDefinition` | Authored program and field types/layout within a base | `MaterialDefinition` |
| `Material` | Concrete shared scene component with independently mutable values/resource bindings | `MaterialInstance` |

`MaterialBase()` selects the ordinary surface contract without emission
extraction or transmission. The room follow-up adds
`MaterialBase.extracted_emissive()` for opaque static meshes whose fragment
output is radiance shared by the visible and extraction passes. A transmissive
base and a more general surface-plus-separate-emission contract remain pending.
Visible-only emissive/unlit shading is definition behavior,
not another base. The definition decides lighting interpretation, including
whether illumination can brighten beyond authored albedo. Scene Bloom/pass
activation remains separate from a base's extraction support.

Construct a definition from a base and a material from a definition. Proposed
syntax, not currently supported MMS API:

```mms
import { f32, vec2, vec3 } from "types"

let definition = MaterialDefinition(MaterialBase())
    .fragment(fragment_shader_uri)
    .inputs({
        burn_progress: f32
        airflow: f32
        burn_direction: vec2
        glow_color: vec3
    })

let cigarette = Material(definition)
    .inputs({
        burn_progress = 0.0
        airflow = 0.2
        burn_direction = [1.0, 0.0]
        glow_color = [1.0, 0.2, 0.0]
    })

let independent_cigarette = Material(definition)
    .inputs({
        burn_progress = 0.4
        airflow = 0.1
        burn_direction = [1.0, 0.0]
        glow_color = [1.0, 0.2, 0.0]
    })
```

The definition consumes a type-only structural expression: colon fields name
types, and resolution produces its schema/type identity. The material consumes
ordinary initial field values using `=`, validated against that schema. This
supersedes the earlier proposal that both arguments were ordinary tables with
equals-style fields storing tags or values. Names and types must match the
shader interface and base contract. These are named material inputs, not a
literal Vulkan descriptor-set description. Resource types, layout metadata and
the precise validation API remain open. The engine owns GPU packing/alignment
and renderer-reserved bindings. See the input/storage distinction below.

Initial schema tags are `f32`, `vec2`, `vec3` and `vec4`; `mat4` is a candidate
after matrix ordering/layout is specified. See the
[MMS type-tag/default/nullability contract](../../crates/meow-meow-script/docs/draft/type-system-epic.md#runtime-type-tag-direction-2026-10-14).
Tags are first-class values distinct from numbers even if internally backed by
numeric IDs. Supporting these schema tags does not commit the first custom
shader upload proof to all those GPU field types; that proof remains f32-only.
It also does not claim native vector/matrix arithmetic or precise vector
`typeof` for an ordinary array.

The convenient overload takes a base directly and creates an instance with
an implicit definition:

```mms
let std_material = Material(MaterialBase())

let animated_material = Material(MaterialBase())
    .fragment(fragment_shader_uri)
    .inputs({
        time_seconds = 0.0
    })
```

Without customization, use the standard default shading implementation for
the selected base. `.fragment(...)` configures the implicit definition, while
`.inputs(...)` on a Material always supplies values. This shorthand
must obtain its field types from the shader interface/reflection and validate
values against them, not infer the declared shader layout from sample values.
It must not mutate another material's definition or a built-in preset.

These constructor arguments are retained references, not structural parent
edges or copies of the referenced object's mutable state. Bases and definitions
must be usable without insertion into the scene tree. Only Material selects
appearance through scene attachment. Every `Material(definition)` or
`Material(base)` creates a new material-instance identity; reusing its live
handle shares that instance. Independent instances can share definition,
program and pipeline resources. Definition mutation/reload and compatibility
rules after instances exist remain to be specified.

Typed record literals carrying both field types and values may eventually serve
either consumer through a defined type/data view. Candidate field syntax is
`progress: f32 = 0.0`, but its grammar, view selection, equality/compatibility
and construction/default rules remain open in the linked proposal. Do not
silently turn a pure schema's fields into initialized material values.

Defaults and current values are separate from field types. The exact policy for
omitted material fields remains to be specified; numeric defaults, if used,
are their type's zero value, and resource/other fields without a defined default
require an initial value. Numeric fields cannot contain null. Only functions,
tables and lists may explicitly allow null under the future MMS type rules;
that language rule does not authorize function-valued GPU uniforms or new
material resource types. `.set_input(name, value)` remains the proposed explicit
live update path; no closure executes during draw recording or on the GPU.

## Named inputs and GPU transport (2026-10-25)

Definition and instance authoring should have the same named-input shape:
`MaterialDefinition(base).inputs(schema)` consumes field types, while
`Material(definition).inputs(values)` consumes initial values for those fields.
The definition owns a fixed schema; instance initialization or mutation cannot
add fields or change their types. Expressions in the value record evaluate
during normal MMS execution and retain their results. They are not automatic
per-frame providers. Existing explicit `set_input` / `get_input` operations
address those same names.

The implemented source spelling is `field: f32` for a type-only record and
`field = expression` for an initialized value record. Type records resolve
primitive names/imported aliases and retain their type role through host
transport. Unsupported material field types are rejected explicitly; mixed
records and nested schemas remain future work. Legacy equals-style tables
containing tags do not satisfy MaterialDefinition.inputs.

Vulkan exposes several input mechanisms with different semantics:

| Mechanism | Relationship to material inputs |
| --- | --- |
| Uniform/storage buffers and sampled resources through descriptors | Current custom f32 values occupy one uniform buffer at set 1, binding 0. A descriptor references the buffer; individual fields are not separate descriptors. |
| Push constants | Can carry small draw-time values without a descriptor set. A future base/program contract could expose material fields this way; the current custom loader explicitly rejects authored push constants. |
| Vertex/instance attributes | Supply geometry/draw-instance data. They are not independent mutable values for each object merely because several objects share one Material. |
| Inter-stage inputs such as fragment UVs | Supplied by preceding shader stages, not initialized by a material value record. |
| Specialization constants | Select specialized program/pipeline variants. They are not ordinary live material fields and must not be changed through a cheap scalar setter. |

References: Khronos [Mapping Data to Shaders](https://docs.vulkan.org/guide/latest/mapping_data_to_shaders.html)
and [Push Constants](https://docs.vulkan.org/guide/latest/push_constants.html).

Keep three concerns separate: field name/type, value ownership/update rate,
and GPU transport/layout. A primitive tag such as f32 states the field type;
it does not select a descriptor, push-constant range, vertex location or rate.
The material's fields are shared material-instance values. Per-object,
per-vertex, frame/view and renderer-owned inputs need their own explicit
contracts/providers; naming the public method `inputs` does not grant arbitrary
access to all shader inputs or add another material identity layer.

The base/program interface and reflected shader layout determine valid mappings.
Reflection validates the authored schema and supplies offsets/bindings; it
cannot infer ownership/update rate from a float or silently move a field from a
UBO to push constants without a compatible shader interface. Custom raw GLSL
still declares the supported layout. An alternate transport requires its own
validated renderer/program implementation, not just a different MMS value.

The common-flow slice preserves the existing bounded f32 UBO path and separates
logical schema/value storage from Anime-specific packing and physical layout.
Built-in and custom sources now use the same registration/resolution/value
update model; fixed built-in and reflected custom packing remain renderer adapters.
Introducing push constants, per-object attributes, specialization controls,
resource inputs or richer field types is not required for that migration.
The public `.inputs(...)` API validates schema versus values and shader
compatibility. The room has migrated; older spellings remain compatibility
entry points while other callers migrate deliberately.

## Goal

Make a stable authored `Material` component the common anchor for built-in and
custom materials. Resolve it alongside existing `Shading`, color, textures,
emissive intensity, transparency/cutout, and phase controls into one validated
draw contract. Keep program definitions, mutable instances, and GPU pipelines
as distinct identities.

## Work and gates

Required attachment forms mirror `Shading`: Material can wrap descendants or
be a direct child of a renderable; local declarations override inherited scopes.
Container-child scopes also follow the shading convention. A wrapper retains
one shared instance, not one copy per inherited renderable.

Each instantiated Material component owns exactly one logical renderer material
instance, regardless of its parameter values. A separate constructor invocation
owns a different instance even when all values match. Sharing the same source
shares its instance; bindings/projections do not count as new material sources.

Structural ECS components remain single-parent; shared leaf Anime/Toon Shading
declarations now preserve multiple attachments and passed both VR checkpoints.
The public Material component must adopt that
[multi-parent attachment contract](multi-parent-components-and-material-sharing.md)
so repeated live child attachments preserve both edges. Broader spatial sharing
is separate. Proposed Material attachment syntax, not implemented API:

```mms
let shared = Material(MaterialBase())
    .fragment("assets/shaders/animated-led-strip.frag")
    .descriptor_set({ time_seconds = 0.0 })
R.cube() { shared }
R.sphere() { shared }
```

Wrapper/direct-child declaration remains the ordinary topology form. A future
`.material(shared)` reference is optional and needs precedence/conflict rules
through the same resolver. Decide source-removal fallback and serialize durable
references. `Material` is already the instance constructor: there is no fourth
layer or separate public `MaterialInstance` constructor. No parameter-equality
algorithm is needed to establish sharing.
For the selected multi-parent path, test repeated live child attachment itself
and edge-specific removal in addition to renderer identity.

For LED-only CombineMesh, all rectangles inherit the wrapper's material. Resolve
the first source's effective material before selecting the combined output's
appearance, and retain that material instance after source geometry collapses.
The backing stays outside the scope. Verify live updates and pending-to-ready
program transitions on the combined output; a static handle/parameter copy is
insufficient.

- [ ] Inventory current components, resolution rules, generated GLTF projections,
  `MaterialHandle` branches, descriptors, vertex interfaces, pipeline/state keys,
  and per-view resources. Link to the existing material/resource-graph ticket.
- [ ] Specify definition/instance/resolved-record schemas, capability constraints,
  authored vs renderer-owned fields, retained generation, and serialization.
- [ ] Implement the f32-only constructor/schema slice with MMS-authored custom
  field names/count using existing grammar, independent of type-expression work.
  Preserve the definition/schema versus instance/value ownership above.
- [ ] For the later colon-style generic API, resolve the record-role design gate
  and implement the MMS type-expression/resolution foundation; freeze the exact
  descriptor-table shape and finite `f32` setter/readback validation. Full
  source annotations/static checking are not prerequisites. Phase 1 GPU fields
  support f32 only; vectors/colors, matrices, integer/boolean fields,
  arrays/structs, resource inputs, and stored callback bindings are later work.
- [ ] Specify material wrapper/local selection, reuse of one live handle,
  independent instances, complete replacement, granular overrides/reset,
  same-scope conflict diagnostics, removal/reparent invalidation, and GLTF
  source identity. Publish a compatibility table for all existing controls.
- [ ] Test wrapper and direct-child forms, inherited material resolution for
  combined source geometry, and first-source instance transfer to the generated
  primitive without rebaking on parameter updates.
- [ ] Specify/test live multi-parent attachments for cross-subtree sharing:
  one Material used by two renderables preserves both bindings and one logical
  renderer instance. Equal-valued separately instantiated Materials stay distinct.
- [ ] Define `Shading` as a built-in surface configuration within this resolver;
  reject incompatible custom-fragment plus built-in surface declarations.
  Decide model-specific property ownership explicitly.
- [ ] Register the component/builders/live methods in MMS and route mutation
  through validated retained state. No script callback executes inside rendering.
- [ ] Implement a compatibility resolver for existing built-in handles before
  dynamic custom programs; ordinary scalar values must not enter pipeline keys.
- [ ] Adapt the custom-fragment first slice to this instance/program ownership
  and reuse its async fallback, bounded-upload, batching, and cleanup gates.

Gate: existing shading/GLTF and specialized-material scenes keep their intended
behavior; material instance sharing and isolation are explicit; local overrides,
source removal/reparent, serialization, conflicts, and invalid inputs have
deterministic coverage. A scalar upload needs no mesh or pipeline rebuild.

Do not remove granular style components or rewrite every renderer material
family just to create the anchor. Record unsupported combinations and migration
boundaries in the material spec before adding new renderer capabilities.
