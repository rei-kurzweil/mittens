// Anime material preset for walls, architecture, vehicles, and other broad
// surfaces. Compared with bisket_anime_shading, it uses a deeper shadow and a
// much wider light-to-shade ramp so large faces retain visible gradation.
export fn environment_anime_shading() {
    return Shading.anime().shade_color([0.22, 0.24, 0.38])
        .shade_strength(0.78)
        .shade_threshold(0.25)
        .lit_threshold(1.25)
        .rim_color([0.82, 0.88, 1.0])
        .rim_strength(0.22)
        .rim_power(5.0)
}
