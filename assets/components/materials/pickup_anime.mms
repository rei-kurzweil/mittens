// Pickup-specific Anime preset. Character presets remain unchanged.
export fn pickup_anime_material() {
    return Material(MaterialDefinition.anime()).inputs({
        shade_color = [0.32, 0.30, 0.48]
        shade_strength = 0.58
        shade_threshold = 0.22
        lit_threshold = 0.85
        rim_color = [0.90, 0.94, 1.0]
        // Ship avatars use 0.42: this is about 2.38 times their rim strength.
        rim_strength = 1.0
        rim_power = 3.5
    }) { name = "pickup_anime_material" }
}
