import { f32 } from "types"

// Flat, textured UI colors independent of scene lighting and bloom.
export fn ui_unlit_material() {
    let definition = MaterialDefinition(MaterialBase())
        .fragment("assets/shaders/ui-unlit.frag")
        .inputs({ brightness: f32 })
    return Material(definition).inputs({ brightness = 1.0 })
}

// One UI material shared by every importer, created on first use, so all
// system panels compile and batch a single custom pipeline.
let shared = {}
export fn shared_ui_unlit_material() {
    if shared["material"] == null { shared["material"] = ui_unlit_material() }
    return shared["material"]
}
