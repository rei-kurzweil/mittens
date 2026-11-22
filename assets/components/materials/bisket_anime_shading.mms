import { f32, vec3 } from "types"

// Canonical albedo-derived anime material for Bisket/mitten avatars, as a
// generic custom definition. Share one definition per opaque bucket so scenes
// compile it once; omit the bucket for the default ("middle").
let shared = {}
export fn bisket_anime_definition(bucket) {
    let key = "default"
    if bucket { key = bucket }
    if shared[key] == null {
        if bucket {
            shared[key] = MaterialDefinition(MaterialBase()).opaque_bucket(bucket)
                .fragment("assets/shaders/anime-cutout.frag")
                .inputs({
                    shade_color: vec3
                    shade_strength: f32
                    shade_threshold: f32
                    lit_threshold: f32
                    rim_color: vec3
                    rim_strength: f32
                    rim_power: f32
                })
        } else {
            shared[key] = MaterialDefinition(MaterialBase())
                .fragment("assets/shaders/anime-cutout.frag")
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
    }
    return shared[key]
}

export fn bisket_anime_shading(bucket) {
    return Material(bisket_anime_definition(bucket)).inputs({
        shade_color = [0.4, 0.4, 0.65]
        shade_strength = 0.5
        shade_threshold = 0.4
        lit_threshold = 0.55
        rim_color = [1.0, 1.0, 1.0]
        rim_strength = 0.38
        rim_power = 4.0
    })
}
