// Canonical albedo-derived anime material settings for Bisket.
// Keep the explicit Toon instance in examples/shading-models.mms unmodified
// so that scene remains a direct pipeline comparison.
export fn bisket_anime_shading() {
    return Shading.anime().shade_color([0.4, 0.4, 0.65])
        .shade_strength(0.5)
        .shade_threshold(0.4)
        .lit_threshold(0.55)
        .rim_color([1.0, 1.0, 1.0])
        .rim_strength(0.38)
        .rim_power(4.0)
}
