// A freestanding 4x2.4 m test board with three bands of increasing detail.
// Local origin is the floor below the board; move/rotate the prefab as a unit.
import { f32 } from "types"
import { anime_surface } from "../materials/anime_surface.mms"

export fn foveation_checkerboard() {
    let definition = MaterialDefinition(MaterialBase()).fragment("assets/shaders/foveation-checkerboard.frag")
        .inputs({ brightness: f32 })
    return T {
        name = "shell_foveation_checkerboard"
        Raycastable.disabled()
        CombineMesh {
            anime_surface()
            C.rgba(0.64,0.46,0.34,1)
            for x in [-2.06,2.06] {
                T.position(x,1.65,0).scale(0.12,2.64,0.12) { R.cube() {} }
            }
            for y in [0.39,2.91] {
                T.position(0,y,0).scale(4.24,0.12,0.12) { R.cube() {} }
            }
            for x in [-1.6,1.6] {
                T.position(x,0.23,0).scale(0.12,0.46,0.12) { R.cube() {} }
                T.position(x,0.04,0).scale(0.5,0.08,0.7) { R.cube() {} }
            }
        }
        T.position(0,1.65,0.07).scale(4,2.4,1) {
            R.square() { Material(definition).inputs({ brightness = 1.0 }) }
        }
    }
}
