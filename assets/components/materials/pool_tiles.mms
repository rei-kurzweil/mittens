// Shared pool ceramic definition and defaults, also used by indoor_pool.
// RGB tangent normals + alpha albedo: assets/textures/pool/white-tiles-surface.png
import { f32 } from "types"

export fn pool_tile_material() {
    let definition = MaterialDefinition(MaterialBase())
        .fragment("assets/shaders/pool-tiles.frag")
        .inputs({
            tile_size: f32
            normal_strength: f32
            specular_strength: f32
            shininess: f32
            rim_strength: f32
            shade_threshold: f32
            lit_threshold: f32
        })
    return Material(definition).inputs({
        tile_size = 0.25
        normal_strength = 1.0
        specular_strength = 0.65
        shininess = 64.0
        rim_strength = 0.2
        shade_threshold = 0.05
        lit_threshold = 2.0
    })
}
