import { f32 } from "types"

// One shared definition; instances retain their own editable finish parameters.
let definition = MaterialDefinition(MaterialBase())
    .fragment("assets/shaders/pickup-painted-metal.frag")
    .inputs({ matcap_strength: f32, specular_strength: f32, shininess: f32,
        rim_strength: f32, rim_power: f32 })

export fn pickup_painted_metal() {
    return Material(definition).inputs({
        matcap_strength = 0.50
        specular_strength = 1.10
        shininess = 56.0
        rim_strength = 0.80
        rim_power = 2.8
    })
}
