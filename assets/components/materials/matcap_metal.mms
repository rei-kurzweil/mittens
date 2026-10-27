// The same shader and matcap used by the pool's chrome handrails.
import { f32 } from "types"
export fn matcap_metal() {
    let definition = MaterialDefinition(MaterialBase()).fragment("assets/shaders/pool-metal-matcap.frag").inputs({ brightness: f32 })
    return Material(definition).inputs({ brightness = 1.0 })
}
