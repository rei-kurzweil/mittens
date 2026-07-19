import { tripod_light } from "../assets/components/tripod_light.mms"

// A side-by-side gallery of every engine light component. Each physical light
// fixture sits between the camera and the non-emissive white target it lights.
RendererSettings { window_size(1440, 810) }
BGC.rgba(0.018, 0.022, 0.035, 1.0)

RenderGraph {
    EmissivePass { BlurPass { radius_ndc(0.025) half_res(true) } }
    Bloom { intensity(0.45) radius_ndc(0.025) emissive_scale(1.0) half_res(true) }
}

fn target_card(card_name, label, x) {
    return T.position(x, 1.45, -1.2) {
        name = card_name
        T.scale(1.05, 1.05, 0.10) {
            // Deliberately non-emissive: all visible shading comes from lights.
            R.cube() { C.rgba(1.0, 1.0, 1.0, 1.0) }
        }

        // Emissive label: white backing with black text remains readable even
        // when its target receives little or no direct light.
        T.position(0.0, -1.42, 0.0) {
            T.scale(1.35, 0.30, 0.06) {
                R.cube() { C.rgba(1.0, 1.0, 1.0, 1.0) EM.on() }
            }
            T.position(-0.54, -0.10, 0.07).scale(0.065, 0.065, 1.0) {
                TXT { label C.rgba(0.0, 0.0, 0.0, 1.0) EM.on() }
            }
        }
    }
}

let ambient_x = -5.4
let directional_x = -1.8
let point_x = 1.8
let spot_x = 5.4
let target = [0.0, 1.45, -1.2]

target_card("ambient_target", "AmbientLight", ambient_x)
target_card("directional_target", "DirectionalLight", directional_x)
target_card("point_target", "PointLight", point_x)
target_card("spot_target", "SpotLight", spot_x)

// AmbientLight is global, so its low base level is visible on every card.
tripod_light("ambient_fixture", [ambient_x, 0, 2.2], [ambient_x, target[1], target[2]],
    AL.rgb(0.10, 0.10, 0.10))
tripod_light("directional_fixture", [directional_x, 0, 2.2], [directional_x, target[1], target[2]],
    DL.color(1.0, 0.84, 0.62).intensity(0.9))
tripod_light("point_fixture",   [point_x, 0, 2.2], [point_x, target[1], target[2]],
    PL.color(0.55, 0.75, 1.0).intensity(6.0).distance(6.5))
tripod_light("spot_fixture",    [spot_x, 0, 2.2], [spot_x, target[1], target[2]],
    SL.color(1.0, 0.48, 0.72).intensity(5.0).distance(7.0).angle(0.30).penumbra(0.25))

// Dark stage floor catches the point and spot falloff without competing with cards.
T.position(0.0, -2.04, 0.0).scale(15.0, 0.12, 10.0) {
    R.cube() { C.rgba(0.055, 0.060, 0.075, 1.0) }
}

// Movable overview camera. Camera3D looks along local -Z.
I.speed(2.5) {
    name = "lights_camera_input"
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }
    T.position(0.0, 1.15, 11.0) {
        name = "lights_camera_rig"
        C3D {
            name = "lights_camera"
            Pointer {}
        }
    }
}
