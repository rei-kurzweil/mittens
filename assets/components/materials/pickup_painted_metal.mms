import { f32 } from "types"

// One shared definition; instances retain their own editable finish parameters.
let definition = MaterialDefinition(MaterialBase())
    .fragment("assets/shaders/pickup-painted-metal.frag")
    .inputs({ specular_strength: f32, shininess: f32,
        rim_strength: f32, rim_power: f32, exposure: f32 })

export fn pickup_painted_metal() {
    return Material(definition).inputs({
        specular_strength = 1.10
        shininess = 56.0
        rim_strength = 0.80
        rim_power = 2.8
        exposure = 0.0
    })
}

// Lit plastic/paint finish with a single view-dependent silhouette rim.
export fn pickup_lit_finish() {
    return Material(definition).inputs({
        specular_strength = 0.80
        shininess = 40.0
        rim_strength = 1.20
        rim_power = 2.5
        exposure = 0.80
    })
}
