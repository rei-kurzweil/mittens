import { f32, vec3 } from "types"

// Stop positions are vertical directions: -1 = nadir, 0 = horizon, +1 = zenith.
// This plain Background is independent of lights and contributes no bloom.
export fn gradient_skybox(low_color, high_color, low_point, high_point) {
    let definition = MaterialDefinition(MaterialBase())
        .fragment("assets/shaders/sky-gradient.frag")
        .inputs({ low_color: vec3
            low_point: f32
            high_color: vec3
            high_point: f32 })
    let material = Material(definition).inputs({
        low_color = low_color
        low_point = low_point
        high_color = high_color
        high_point = high_point
    })
    return BG {
        name = "gradient_skybox"
        T.scale(80.0, 80.0, 80.0) {
            R.sphere() { material }
        }
    }
}

// Keep the sun separate from the plain sky so only its emitted light blooms.
// Occlusion+lighting enables background extraction; EM bypasses surface lighting.
export fn background_sun(direction, angular_radius, color, intensity) {
    let length = Math.sqrt(direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2])
    if length < 0.0001 { length = 1.0 }
    let radius = 50.0
    let sun_size = 2.0 * Math.sin(angular_radius) * radius
    return BG.occlusion_and_lighting() {
        name = "background_sun"
        T.position(direction[0] / length * radius, direction[1] / length * radius, direction[2] / length * radius)
            .scale(sun_size, sun_size, sun_size) {
            R.sphere() {
                Material(MaterialDefinition.toon())
                C.rgba(color[0], color[1], color[2], 1.0)
                EM.on().intensity(intensity)
            }
        }
    }
}
