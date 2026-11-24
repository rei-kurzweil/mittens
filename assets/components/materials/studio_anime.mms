// Studio architecture is ordinary depth-writing geometry, not Background.
// Keep per-object Color inputs and the standard Anime defaults. Created on
// first use, so importers that pass their own material (the Android renderer
// accepts only custom definitions) never register the built-in one.
let shared = {}

export fn studio_environment_material() {
    if shared["material"] == null {
        shared["material"] = Material(MaterialDefinition.anime().opaque_bucket("far"))
    }
    return shared["material"]
}
