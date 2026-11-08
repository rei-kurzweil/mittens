import { anime_directional_fill_definition } from "./anime_directional_fill.mms"

// Station-specific settings. Shared Anime defaults remain suitable for other scenes.
export fn ship_environment_material(bucket) {
    if bucket == null { bucket = "far" }
    return Material(MaterialDefinition.anime().opaque_bucket(bucket)).inputs({
        shade_color = [0.15, 0.17, 0.30]
        shade_strength = 0.82
        shade_threshold = 0.12
        lit_threshold = 1.60
        rim_color = [0.82, 0.88, 1.0]
        rim_strength = 0.90
        rim_power = 2.4
    }) { name = "ship_environment_anime_material" }
}

// Character-only experiment: overhead lights cannot shade back-facing surfaces.
// Faces and clothing retain more light and a narrower transition than walls.
export fn ship_avatar_material() {
    return Material(anime_directional_fill_definition()).inputs({
        shade_color = [0.32, 0.30, 0.48]
        shade_strength = 0.58
        shade_threshold = 0.22
        lit_threshold = 0.85
        rim_color = [1.0, 0.94, 1.0]
        rim_strength = 0.42
        rim_power = 3.5
        unshaded_axis = [0.0, 1.0, 0.0]
        unshaded_min_elevation_degrees = 25.0
    }) { name = "ship_avatar_anime_material" }
}

// Face-only fill: retain the texture's colours under overhead kiosk lighting.
export fn ship_face_material() {
    return Material(anime_directional_fill_definition()).inputs({
        shade_color = [0.72, 0.65, 0.73]
        shade_strength = 0.28
        shade_threshold = 0.22
        lit_threshold = 0.85
        rim_color = [1.0, 0.94, 1.0]
        rim_strength = 0.42
        rim_power = 3.5
        unshaded_axis = [0.0, 1.0, 0.0]
        unshaded_min_elevation_degrees = 25.0
    }) { name = "ship_face_anime_material" }
}
