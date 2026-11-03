import { f32, vec3 } from "types"

// Generic custom definition; the application renderer needs no Anime handle.
export fn anime_surface() {
    let definition = MaterialDefinition(MaterialBase())
        .fragment("assets/shaders/anime-surface.frag")
        .inputs({
            shade_color: vec3
            shade_strength: f32
            shade_threshold: f32
            lit_threshold: f32
            rim_color: vec3
            rim_strength: f32
            rim_power: f32
        })
    return Material(definition).inputs({
        shade_color = [0.42, 0.43, 0.52]
        shade_strength = 0.6
        shade_threshold = 0.1
        lit_threshold = 1.6
        rim_color = [0.85, 0.9, 1.0]
        rim_strength = 0.10
        rim_power = 4.0
    })
}
