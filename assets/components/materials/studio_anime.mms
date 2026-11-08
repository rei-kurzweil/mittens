// Studio architecture is ordinary depth-writing geometry, not Background.
// Keep per-object Color inputs and the standard Anime defaults.
let studio_material = Material(MaterialDefinition.anime().opaque_bucket("far"))

export fn studio_environment_material() {
    return studio_material
}
