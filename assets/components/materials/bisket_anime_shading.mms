// Canonical albedo-derived anime material settings for Bisket.
// Keep the explicit Toon instance in examples/shading-models.mms unmodified
// so that scene remains a direct pipeline comparison.
export fn bisket_anime_shading() {
    return Shading.anime().shade_color([0.28,0.22,0.44])
        .shade_strength(0.68)
        .shade_threshold(0.15)
        .lit_threshold(0.90)
        .rim_color([1.0, 1.0, 1.0])
        .rim_strength(0.95)
        .rim_power(2.3)
}
