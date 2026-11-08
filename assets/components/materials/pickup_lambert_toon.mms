import { lambert_toon_definition } from "./lambert_toon.mms"

// Match pickup Anime controls so the comparison changes only the diffuse response.
export fn pickup_lambert_toon_material() {
    return Material(lambert_toon_definition()).inputs({
        shade_color = [0.32, 0.30, 0.48]
        shade_strength = 0.58
        shade_threshold = 0.22
        lit_threshold = 0.85
        rim_color = [0.90, 0.94, 1.0]
        // Ship avatars use 0.42: this is about 2.38 times their rim strength.
        rim_strength = 1.0
        rim_power = 3.5
    }) { name = "pickup_lambert_toon_material" }
}
