import { f32 } from "types"

// Explicit unshaded glow with emission extraction, independent of surface shading.
let definition = MaterialDefinition(MaterialBase.extracted_emissive())
    .fragment("assets/shaders/emissive-surface.frag")
    .inputs({ gain: f32 })

export fn emissive_surface() {
    return Material(definition).inputs({ gain = 1.0 })
}
