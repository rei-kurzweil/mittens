// Pink Sakura sky shared by the PC shell and the native headset scene.
import { f32, vec3 } from "types"
import { ui_unlit_material } from "../materials/ui_unlit.mms"

export fn sakura_sky(with_moon) {
    let definition = MaterialDefinition(MaterialBase()).fragment("android/shaders/sakura-sky.frag")
        .inputs({ low_color: vec3, low_point: f32, high_color: vec3, high_point: f32,
            sun_direction: vec3, sun_angular_radius: f32, sun_color: vec3, sun_strength: f32 })
    let sun_strength = 1.0
    if with_moon { sun_strength = 0.0 }
    return BG {
        name = "shell_sakura_sky"
        T.scale(80,80,80) {
            R.sphere() { Material(definition).inputs({
                low_color = [1.0, 0.80, 0.86], low_point = -0.15,
                high_color = [0.90, 0.55, 0.73], high_point = 0.85,
                sun_direction = [-0.65, 0.47, -1.0], sun_angular_radius = 0.055,
                sun_color = [1.0, 0.92, 0.76], sun_strength = sun_strength
            }) }
        }
        if with_moon {
            // A real concave crescent polygon, sharing the rotation-only sky.
            // Its custom unlit material needs no scene lights or bloom pass.
            T.position(23,24,-50).scale(3,3,3) {
                name = "shell_moon"
                R.polygon("mitten-droid/crescent-moon/v1", [
                    [0.500000, 0.866025],
                    [0.342020, 0.939693],
                    [0.173648, 0.984808],
                    [0.000000, 1.000000],
                    [-0.173648, 0.984808],
                    [-0.342020, 0.939693],
                    [-0.500000, 0.866025],
                    [-0.642788, 0.766044],
                    [-0.766044, 0.642788],
                    [-0.866025, 0.500000],
                    [-0.939693, 0.342020],
                    [-0.984808, 0.173648],
                    [-1.000000, 0.000000],
                    [-0.984808, -0.173648],
                    [-0.939693, -0.342020],
                    [-0.866025, -0.500000],
                    [-0.766044, -0.642788],
                    [-0.642788, -0.766044],
                    [-0.500000, -0.866025],
                    [-0.342020, -0.939693],
                    [-0.173648, -0.984808],
                    [-0.000000, -1.000000],
                    [0.173648, -0.984808],
                    [0.342020, -0.939693],
                    [0.500000, -0.866025],
                    [0.400262, -0.842693],
                    [0.303916, -0.807915],
                    [0.212271, -0.762163],
                    [0.126570, -0.706060],
                    [0.047979, -0.640368],
                    [-0.022435, -0.565978],
                    [-0.083717, -0.483901],
                    [-0.135033, -0.395251],
                    [-0.175687, -0.301234],
                    [-0.205126, -0.203125],
                    [-0.222951, -0.102257],
                    [-0.228920, -0.000000],
                    [-0.222951, 0.102257],
                    [-0.205126, 0.203125],
                    [-0.175687, 0.301234],
                    [-0.135033, 0.395251],
                    [-0.083717, 0.483901],
                    [-0.022435, 0.565978],
                    [0.047979, 0.640368],
                    [0.126570, 0.706060],
                    [0.212271, 0.762163],
                    [0.303916, 0.807915],
                    [0.400262, 0.842693]
                ]) {
                    ui_unlit_material()
                    C.rgba(1.0,0.96,0.86,1.0)
                }
            }
        }
    }
}
