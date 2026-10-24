# Task: Material component and resolved material contract

Date: 2026-10-09
Status: custom-f32 component/API subset implemented 2026-10-24; full generic contract below remains proposed.
Parent: [custom materials epic](epic/custom-materials.md).
Release: [0.10.0](epic/0.10.0/README.md).

Implemented subset: [youtuber-room checkpoint](youtuber-room-custom-material-checkpoint.md).
`MaterialBase()`, `MaterialDefinition(base).fragment(path).input_f32(name)` and
`Material(definition).descriptor_set(values)` register directly from MMS without
requiring scene attachment. The default opaque mesh base and
`MaterialBase.extracted_emissive()`, local GLSL files and named scalar-f32 fields
are supported. The colon-style schemas and `Material(base)`
implicit-definition overload below remain future API. Built-in definitions still
need the [common material-flow migration](material-instances-and-family-contracts.md#first-refactoring-slices-and-gates),
regardless of whether they remain authored in Rust/GLSL.

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
revises definition schemas to colon-style type expressions, with commas allowed.
That `TypeExpression` AST/resolution foundation remains pending; primitive tags
alone do not implement it. The later first-slice decision allows a current-grammar
f32 declaration builder or names-only list on MaterialDefinition: each MMS author
chooses field names/count, every field is f32, and shader reflection validates
the schema/layout. This narrow API can precede type expressions and the open
record-role semantics. The colon-style examples below describe the eventual
generic API. No code changes are part of this revision.

## Authoring decision (2026-10-23)

There are exactly three layers, with the following public MMS names. This
supersedes the earlier Material-as-definition / MaterialInstance authoring
proposal and the combined `Material.custom(...).input_f32(...)` examples.
Current Rust source names remain unchanged in this documentation pass.

| MMS type | Responsibility | Current Rust record |
| --- | --- | --- |
| `MaterialBase` | Read-only engine-provided renderer interfaces, features, state and resource contract | `MaterialFamily` |
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
    .descriptor_set({
        burn_progress: f32,
        airflow: f32,
        burn_direction: vec2,
        glow_color: vec3,
    })

let cigarette = Material(definition)
    .descriptor_set({
        burn_progress = 0.0
        airflow = 0.2
        burn_direction = [1.0, 0.0]
        glow_color = [1.0, 0.2, 0.0]
    })

let independent_cigarette = Material(definition)
    .descriptor_set({
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
shader interface and base contract. This record
shape is illustrative: literal descriptor-set/binding numbers, nested tables,
resource entries, layout metadata and the precise validation API remain open.
The engine owns GPU packing/alignment and renderer-reserved bindings.

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
    .descriptor_set({
        time_seconds = 0.0
    })
```

Without customization, use the standard default shading implementation for
the selected base. `.fragment(...)` configures the implicit definition, while
`.descriptor_set(...)` on a Material always supplies values. This shorthand
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
