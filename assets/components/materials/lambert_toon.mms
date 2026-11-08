import { f32, vec3 } from "types"

// One shared definition; materials supply their own ramp and rim parameters.
let shared = {near = null, middle = null, far = null}
export fn lambert_toon_definition(bucket) {
    if bucket == null { bucket = "middle" }
    if shared[bucket] == null {
        shared[bucket] = MaterialDefinition(MaterialBase()).opaque_bucket(bucket)
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
    return shared[bucket]
}
