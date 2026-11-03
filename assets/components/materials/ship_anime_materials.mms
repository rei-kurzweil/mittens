// Station-specific settings. Shared Anime defaults remain suitable for other scenes.
export fn ship_environment_material() {
    return Material(MaterialDefinition.anime()).inputs({
        shade_color = [0.15, 0.17, 0.30]
        shade_strength = 0.82
        shade_threshold = 0.12
        lit_threshold = 1.60
        rim_color = [0.82, 0.88, 1.0]
        rim_strength = 0.90
        rim_power = 2.4
    }) { name = "ship_environment_anime_material" }
}

// Faces and clothing retain more light and a narrower transition than walls.
export fn ship_avatar_material() {
    return Material(MaterialDefinition.anime()).inputs({
        shade_color = [0.32, 0.30, 0.48]
        shade_strength = 0.58
        shade_threshold = 0.22
        lit_threshold = 0.85
        rim_color = [1.0, 0.94, 1.0]
        rim_strength = 0.42
        rim_power = 3.5
    }) { name = "ship_avatar_anime_material" }
}
