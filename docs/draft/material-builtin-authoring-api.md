# Built-in material authoring API

Status: implemented 2026-10-26; legacy Shading remains available.

See [typed material checkpoint](../task/materials-v3-typed-inputs-and-builtins.md)
for implementation and validation details.

Keep the three authored concepts: MaterialBase owns renderer contracts and
resources, MaterialDefinition owns the shader/schema/defaults, and Material owns
instance values. Built-ins use the same Material component as custom
definitions. Shading remains supported as a compatibility authoring adapter.

## Authoring

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

MaterialDefinition.anime() and MaterialDefinition.toon() reference engine-owned
built-in definitions. Repeated calls reuse the authored built-in reference and
the same renderer definition. Creating two Material objects creates independent
instances, even if their initial values are identical.

Keep .inputs(...) and .set_input(name, value) as the common parameter API. Partial
instance inputs inherit definition defaults. Attaching one Material to several
consumers shares its identity and values; attaching a different Material provides
a local override. Existing per-renderable Color/Opacity/Texture inputs keep their
separate ownership and semantics.

## Colors and complete Shading compatibility

Named vector inputs, including colors, use lists of the declared width:

```mms
let character = Material(MaterialDefinition.anime()).inputs({
    shade_color = [0.5, 0.5, 0.5]
    shade_strength = 0.7
    rim_color = [1.0, 1.0, 1.0]
    rim_strength = 0.2
})
```

Material.inputs and set_input validate scalar f32 values or lists containing exactly
2, 3 or 4 finite f32 components against each declared field. get_input returns the
same shape, including definition defaults for unset inputs. These are
schema-validated lists, not new language-level vector value objects.

GLTF-generated primitives resolve Material through their GLTF owner while retaining
imported texture/color inputs. Local material overrides precede the owner scope;
sharing the authored Material preserves one renderer instance identity. Structural
edits refresh generated roots; value edits update the shared instance revision.
Legacy Shading projections remain supported.

## Custom materials keep the current API

```mms
import { f32, vec3 } from "types"

let definition = MaterialDefinition(MaterialBase.planar_reflective())
    .fragment("my-water.frag")
    .inputs({
        time_seconds: f32
        tint: vec3
    })
let water = Material(definition).inputs({
    time_seconds = 0.0
    tint = [0.1, 0.5, 0.7]
})
```

The reflective base supplies capture resources; the fragment program controls
their interpretation. No family layer, public pipeline handle or fourth authored
material concept is introduced. Built-in constructors select their renderer
contract; replacing their fragment/schema requires an explicitly authored custom
definition, rather than mutating a shared engine definition.

## Remaining scope

The existing engine defaults and emission behavior are preserved. Custom schemas
support f32, vec2, vec3 and vec4, matched by name and exact type to GLSL fields.
Logical values are densely stored; reflected byte offsets determine GPU packing.
Nested records, matrices, typed variable annotations and new descriptor kinds
are outside this slice. Reference-preserving serialization and eventual removal
of Shading remain separate migration work.

No Material(base) implicit-definition convenience, custom vertex API or new
transmissive preset is included in this pass.
