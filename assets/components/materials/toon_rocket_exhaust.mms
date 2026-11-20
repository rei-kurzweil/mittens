import { f32, vec3 } from "types"

// Apply to translucent ribbon geometry with UV y=0 at the rocket. Each rocket
// needs its own instance because launch_origin/flight_direction are per shot;
// the compiled definition is shared through module caching.
let definition = MaterialDefinition(MaterialBase())
    .fragment("assets/shaders/toon-rocket-exhaust.frag")
    .inputs({
        launch_origin: vec3
        flight_direction: vec3
        light_color: vec3
        shadow_color: vec3
        wave_frequency: f32
        wave_amplitude: f32
    })

export fn toon_rocket_exhaust(launch_origin, flight_direction) {
    return Material(definition).inputs({
        launch_origin = launch_origin
        flight_direction = flight_direction
        light_color = [0.72, 0.73, 0.74]
        shadow_color = [0.49, 0.50, 0.52]
        wave_frequency = 7.0
        wave_amplitude = 0.08
    })
}
