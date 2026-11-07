import { f32, vec3 } from "types"

// Experimental variant; share one definition and compile only when requested.
// Callers provide the usual Anime parameters plus a world-space unshaded axis
// and an elevation in degrees above the plane perpendicular to that axis.
let shared = {definition = null}
export fn anime_directional_fill_definition() {
    if shared.definition == null {
        shared.definition = MaterialDefinition(MaterialBase())
            .fragment("assets/shaders/anime-directional-fill.frag")
            .inputs({
                shade_color: vec3
                shade_strength: f32
                shade_threshold: f32
                lit_threshold: f32
                rim_color: vec3
                rim_strength: f32
                rim_power: f32
                unshaded_axis: vec3
                unshaded_min_elevation_degrees: f32
            })
    }
    return shared.definition
}
