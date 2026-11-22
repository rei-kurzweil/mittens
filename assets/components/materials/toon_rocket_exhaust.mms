import { f32, vec3 } from "types"

// Apply to a translucent local-XY quad with UV y=0 at the rocket. The axial
// billboard base keeps the quad's local Y along the flight line and turns it
// about that line toward each view, so one quad replaces two crossed planes.
// Each rocket needs its own instance because launch_origin/flight_direction
// are per shot; the compiled definition is shared through module caching.
let definition = MaterialDefinition(MaterialBase.axial_billboard())
    .fragment("assets/shaders/toon-rocket-exhaust.frag")
    .inputs({
        launch_origin: vec3
        flight_direction: vec3
        light_color: vec3
        shadow_color: vec3
        wave_frequency: f32
        wave_amplitude: f32
        trail_length: f32
    })

export fn toon_rocket_exhaust(launch_origin, flight_direction, trail_length) {
    return Material(definition).inputs({
        launch_origin = launch_origin
        flight_direction = flight_direction
        light_color = [0.72, 0.73, 0.74]
        shadow_color = [0.49, 0.50, 0.52]
        wave_frequency = 7.0
        wave_amplitude = 0.08
        trail_length = trail_length
    })
}
