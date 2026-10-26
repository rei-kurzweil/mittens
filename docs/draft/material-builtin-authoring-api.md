# Built-in material authoring: API review

Status: proposal for review; no public authoring migration implemented.

Keep the three authored concepts: MaterialBase owns renderer contracts and
resources, MaterialDefinition owns the shader/schema/defaults, and Material owns
instance values. Built-ins should use the same Material component as custom
definitions. Shading remains supported until the replacement is reviewed and its
compatibility behavior is implemented.

## Proposed authoring

```mms
let anime = MaterialDefinition.anime()
let walls = Material(anime).inputs({
    shade_strength = 0.7
    shade_threshold = 0.05
    lit_threshold = 2.0
    rim_strength = 0.2
    rim_power = 4.0
})

R.cube() {
    C.rgba(0.7, 0.6, 0.8, 1.0)
    walls
}
```

MaterialDefinition.anime() and MaterialDefinition.toon() are proposed constructors,
not available methods yet. They should reference engine-owned built-in definitions;
calling them repeatedly should not compile another shader or allocate another
underlying definition. Creating two Material objects should create independent
instances, even if their initial values are identical.

Keep .inputs(...) and .set_input(name, value) as the common parameter API. Partial
instance inputs inherit definition defaults. Attaching one Material to several
consumers shares its identity and values; attaching a different Material provides
a local override. Existing per-renderable Color/Opacity/Texture inputs keep their
separate ownership and semantics.

## Colors and complete Shading compatibility

The intended complete replacement also needs named color fields:

```mms
let character = Material(MaterialDefinition.anime()).inputs({
    shade_color = [0.5, 0.5, 0.5]
    shade_strength = 0.7
    rim_color = [1.0, 1.0, 1.0]
    rim_strength = 0.2
})
```

This color syntax is also a proposal. Built-in schemas already retain Vec3 fields,
but public Material.inputs currently accepts scalar f32 values only. We must
review and implement schema-checked vector values before claiming a complete
replacement or removing Shading's color builders. Custom vector GPU uploads
remain a separate physical-layout extension.

Built-in material attachment to GLTF should preserve imported texture/color
defaults, local overrides and shared instance identity. The existing Shading
projection adapter must be migrated deliberately; exposing the constructor alone
does not establish complete GLTF compatibility.

## Custom materials keep the current API

```mms
let definition = MaterialDefinition(MaterialBase.planar_reflective())
    .fragment("assets/shaders/pool-water.frag")
    .inputs({ /* named f32 type fields */ })
let water = Material(definition).inputs({ /* named initial values */ })
```

The reflective base supplies capture resources; the fragment program controls
their interpretation. No family layer, public pipeline handle or fourth authored
material concept is introduced. Built-in constructors select their renderer
contract; replacing their fragment/schema requires an explicitly authored custom
definition, rather than mutating a shared engine definition.

## Review decisions before implementation

- Confirm MaterialDefinition.anime()/toon() as the built-in entry points.
- Confirm that instance values, including colors, use .inputs/.set_input rather
  than duplicating every Shading builder on Material.
- Confirm schema-checked three-element lists as values for Vec3 color fields.
- Keep engine defaults and current emission behavior during the API migration;
  changing lighting/emission policy is an independent decision.

No Material(base) implicit-definition convenience, custom vertex API, new
transmissive preset, or removal/deprecation of Shading is included in this pass.
