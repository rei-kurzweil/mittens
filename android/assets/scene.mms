import { f32, vec3 } from "types"
let definition = MaterialDefinition(MaterialBase())
    .fragment("__FRAGMENT_PATH__")
    .inputs({ tint: vec3, gain: f32 })
T.position(-0.35, 0.0, -2.0).scale(0.45, 0.45, 0.45) {
    R.cube() { Material(definition).inputs({ tint = [0.15, 0.8, 1.0], gain = 0.8 }) }
}
T.position(0.35, 0.0, -2.3).scale(0.55, 0.55, 0.55) {
    R.cube() { Material(definition).inputs({ tint = [1.0, 0.3, 0.15], gain = 0.8 }) }
}
