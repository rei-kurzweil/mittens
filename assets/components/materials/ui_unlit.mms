import { f32 } from "types"

// Flat, textured UI colors independent of scene lighting and bloom.
export fn ui_unlit_material() {
    let definition = MaterialDefinition(MaterialBase())
        .fragment("assets/shaders/ui-unlit.frag")
        .inputs({ brightness: f32 })
    return Material(definition).inputs({ brightness = 1.0 })
}
