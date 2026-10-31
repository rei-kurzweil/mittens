# Materials V3: typed inputs and built-in definitions

Date: 2026-10-26
Status: implemented; headset acceptance pending.

## Authoring

```mms
let anime = MaterialDefinition.anime()
let walls = Material(anime).inputs({
    shade_color = [0.35, 0.35, 0.35]
    shade_strength = 0.8
    rim_color = [1.0, 1.0, 1.0]
    rim_strength = 0.18
})
R.cube() { walls }
walls.set_input("rim_color", [0.8, 0.9, 1.0])
```

MaterialDefinition.anime()/toon() return shared, engine-owned built-in definition
references. Shader/schema builders reject edits to these definitions. Separate
Material objects retain independent values; attaching one Material several times
shares one renderer instance. Partial inputs inherit existing engine defaults.
get_input returns overridden values or the definition default, with scalars as
numbers and vectors as lists. Existing Color/Opacity/Texture/Emissive components
retain their standard per-renderable roles, including Toon emission activation.

Custom definitions use the same value API and preserve individual field types:

```mms
import { f32, vec2, vec3, vec4 } from "types"
let definition = MaterialDefinition(MaterialBase())
    .fragment("my-material.frag")
    .inputs({
        gain: f32
        uv_offset: vec2
        tint: vec3
        rgba: vec4
    })
let material = Material(definition).inputs({
    gain = 1.0
    uv_offset = [0.0, 0.0]
    tint = [0.2, 0.5, 0.8]
    rgba = [1.0, 1.0, 1.0, 1.0]
})
```

RecordType already carried primitive TypeTags; this slice extends material
consumption, not the language type-expression AST. Only f32/vec2/vec3/vec4 are
accepted as material field types. Instance values require the exact declared
shape and finite f32 components, including overflow checks. Lists are validated
against the schema; they are not new nominal vector values. The input_f32 and
descriptor_set compatibility aliases continue to work.

## Renderer and scene integration

Authored fields retain names and types. Logical values use dense scalar slots;
custom GLSL reflection validates field names/counts and exact scalar/vector types.
Reflected offsets are expanded to component offsets for uploads. Padding bytes
are zero, including alignment before vec3/vec4 and the rounded block tail. The
same bounded descriptor snapshot/revision path handles scalar and vector edits.
The existing built-in anime UBO adapter still packs logical fields by name.

Imported primitives retain their GLTF owner as a material scope, because their
generated transform hierarchy is outside the GLTF component's ordinary children.
The common resolver honors local overrides, then that owner scope, without
copying the shared Material or replacing imported texture/color inputs. Scope
changes also traverse generated GLTF roots. Legacy Shading projections continue
to resolve through the common material path.

When an enclosing scene subtree still uses legacy construction, inline material
children are constructed through the configured host and attached before the
enclosing subtree initializes. They do not require legacy material constructors.

The youtuber room now uses the built-in Material API for its previous Anime/Toon
declarations, preserving explicit parameter values and all scene geometry and
view switches. Imported pool components can still use legacy Shading.

## Validation

- Release build and formatting checks pass.
- Configured-host type records, scalar/vector values, get/set/defaults, shared
  built-in definitions, read-only schemas, invalid shapes/tags/nonfinite values.
- GLSL mixed-type reflection, reordered fields, vec3 padding, exact component
  uploads, schema/type mismatches and existing scalar custom materials.
- Material lifecycle and room construction in desktop/XR/both modes; CombineMesh
  identity and update behavior.
- GLTF shared bindings, preserved imported texture/color values, local overrides,
  detach/restoration and legacy compatibility on a skinned model.
- Four Vulkan checks, including mixed vector uniform buffers, bounded snapshots,
  stereo room/mitten rendering and existing planar water/capture lifetime checks.

Remaining work includes reference-preserving serialization, migration/removal of
legacy Shading authoring, matrices/nested records and custom deformation shaders.
No general typed variable annotations or implicit table-to-record conversion were
added. A broad integration build also encounters the existing test import of the
removed TransformParentComponent; focused library checks are used for this slice.
