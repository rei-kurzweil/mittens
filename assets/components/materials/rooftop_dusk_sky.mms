import { f32, vec3 } from "types"

let definition = MaterialDefinition(MaterialBase())
    .fragment("assets/shaders/rooftop-dusk-sky.frag")
    .inputs({
        ground_color: vec3, horizon_point: f32,
        horizon_color: vec3, lilac_point: f32,
        lilac_color: vec3, zenith_point: f32,
        zenith_color: vec3, haze_depth: f32
    })

// Numeric colors are linear shader values. Stops are viewing-direction Y.
// Pass false for defaults or a table of overrides.
export fn rooftop_dusk_sky_material(config) {
    let inputs = {
        ground_color = [0.44, 0.36, 0.38], horizon_point = 0.0,
        horizon_color = [0.93, 0.80, 0.66], lilac_point = 0.24,
        lilac_color = [0.68, 0.56, 0.84], zenith_point = 0.95,
        zenith_color = [0.22, 0.17, 0.36], haze_depth = 0.18
    }
    if config {
        for field in ["ground_color", "horizon_point", "horizon_color", "lilac_point",
            "lilac_color", "zenith_point", "zenith_color", "haze_depth"] {
            if config[field] != null { inputs[field] = config[field] }
        }
    }
    return Material(definition).inputs(inputs)
}
