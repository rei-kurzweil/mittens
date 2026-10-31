import { f32, vec2, vec3 } from "types"

// Opaque paper -> ember/char -> ash along an authored UV interval. This colors
// an existing mesh; it does not shorten geometry or enable bloom/extraction.
let definition = MaterialDefinition(MaterialBase())
    .fragment("assets/shaders/burning.frag")
    .inputs({
        burn_progress: f32
        burn_axis: vec2
        burn_start: f32
        burn_end: f32
        ember_width: f32
        char_width: f32
        edge_noise: f32
        time_seconds: f32
        ember_color: vec3
        char_color: vec3
        ash_color: vec3
        shade_strength: f32
    })

export fn burning_material() {
    return Material(definition).inputs({
        burn_progress = 0.0
        burn_axis = [0.0, 1.0]
        burn_start = 0.0
        burn_end = 1.0
        ember_width = 0.025
        char_width = 0.05
        edge_noise = 0.012
        time_seconds = 0.0
        ember_color = [1.0, 0.26, 0.035]
        char_color = [0.09, 0.065, 0.055]
        ash_color = [0.49, 0.47, 0.46]
        shade_strength = 0.22
    })
}
