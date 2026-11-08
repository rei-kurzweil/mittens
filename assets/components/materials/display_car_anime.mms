// Match the display car's existing Bisket Anime lighting without changing the
// character preset. Enterable cars use Near; scenery can request Middle.
let definitions = {near = null, middle = null, far = null}

export fn display_car_anime_material(bucket) {
    if bucket == null { bucket = "near" }
    if definitions[bucket] == null {
        definitions[bucket] = MaterialDefinition.anime().opaque_bucket(bucket)
    }
    return Material(definitions[bucket]).inputs({
        shade_color = [0.4, 0.4, 0.65]
        shade_strength = 0.5
        shade_threshold = 0.4
        lit_threshold = 0.55
        rim_color = [1.0, 1.0, 1.0]
        rim_strength = 0.38
        rim_power = 4.0
    }) { name = "display_car_anime_material" }
}
