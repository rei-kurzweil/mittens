import { f32, vec3 } from "types"

let definition = MaterialDefinition(MaterialBase())
    .fragment("assets/shaders/kawaii-twilight-sky.frag")
    .inputs({
        low_color: vec3, low_point: f32,
        middle_color: vec3, middle_point: f32,
        high_color: vec3, high_point: f32,
        cloud_color: vec3, cloud_strength: f32,
        moon_direction: vec3, moon_radius: f32,
        moon_color: vec3, moon_strength: f32,
        glow_color: vec3, glow_strength: f32,
        moon_rotation: f32
    })

// Numeric colors are linear shader values. Angles are radians; gradient stops
// are viewing-direction Y. Pass false for defaults or a table of overrides.
export fn kawaii_twilight_sky_material(config) {
    let inputs = {
        low_color = [1.0, 0.67, 0.78], low_point = -0.12,
        middle_color = [0.67, 0.53, 0.82], middle_point = 0.28,
        high_color = [0.60, 0.48, 0.78], high_point = 1.0,
        cloud_color = [0.96, 0.80, 0.94], cloud_strength = 0.55,
        moon_direction = [-0.35, 0.30, -0.88], moon_radius = 0.055,
        moon_color = [0.96, 0.91, 1.0], moon_strength = 1.25,
        glow_color = [0.95, 0.65, 0.90], glow_strength = 0.85,
        moon_rotation = -0.25
    }
    if config {
        for field in ["low_color", "low_point", "middle_color", "middle_point",
            "high_color", "high_point", "cloud_color", "cloud_strength",
            "moon_direction", "moon_radius", "moon_color", "moon_strength",
            "glow_color", "glow_strength", "moon_rotation"] {
            if config[field] != null { inputs[field] = config[field] }
        }
    }
    return Material(definition).inputs(inputs)
}
