// The same shader and matcap used by the pool's chrome handrails.
import { f32 } from "types"
// brightness (optional, default 1.0) darkens the reflection, e.g. 0.4 for a
// dark galvanised steel.
export fn matcap_metal(brightness) {
    if brightness == null { brightness = 1.0 }
    let definition = MaterialDefinition(MaterialBase()).fragment("assets/shaders/pool-metal-matcap.frag").inputs({ brightness: f32 })
    return Material(definition).inputs({ brightness = brightness })
}
