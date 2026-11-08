import { f32, vec3 } from "types"

// One shared definition; materials supply their own ramp and rim parameters.
let shared = {definition = null}
export fn lambert_toon_definition() {
    if shared.definition == null {
        shared.definition = MaterialDefinition(MaterialBase())
            .fragment("assets/shaders/lambert-toon.frag")
            .inputs({
                shade_color: vec3
                shade_strength: f32
                shade_threshold: f32
                lit_threshold: f32
                rim_color: vec3
                rim_strength: f32
                rim_power: f32
            })
    }
    return shared.definition
}
