import { f32, vec3 } from "types"

// Shared two-tone appearance. Lifetime opacity belongs to each smoke section's
// Color, so sections of different ages can share this one material instance.
// Module caching shares the compiled definition across independently valued
// materials too; creating another emitter does not compile another program.
let definition = MaterialDefinition(MaterialBase())
    .fragment("assets/shaders/smoke.frag")
    .inputs({
        light_color: vec3
        shadow_color: vec3
        light_direction: vec3
        shade_threshold: f32
        band_softness: f32
        density: f32
    })

export fn smoke_material() {
    return Material(definition).inputs({
        light_color = [0.78, 0.81, 0.86]
        shadow_color = [0.43, 0.47, 0.56]
        light_direction = [-0.4, 0.8, 0.5]
        shade_threshold = 0.12
        band_softness = 0.025
        density = 1.0
    })
}
