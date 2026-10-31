import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { bisket_humanoid_bone_map } from "../assets/components/humanoid_bone_maps/bisket.mms"
import { xr_settings_panel } from "../assets/components/panels/xr_settings_panel.mms"

// Flat-ground lower-body IK smoke scene. Load directly for Bisket; the optional
// Rust example launcher also accepts --model bisket|rei.
RendererSettings.msaa_off() { window_size(800, 600) }
XR.on()
BGC.rgba(0.22, 0.27, 0.33, 1.0)
AL.rgb(0.65, 0.65, 0.7)
T.position(0.0, 2.5, 1.0) { DL { intensity(1.2) color(1.0, 0.97, 0.93) } }

T.position(0.0, -0.05, 0.0).scale(12.0, 0.1, 12.0) {
    name = "walking_floor"
    Zone.cube([0.5, 0.5, 0.5]) { Collidable.static() {} }
    R.cube() { C.rgba(0.42, 0.46, 0.49, 1.0) }
}
// A full-height mirror lets you watch both feet and knees from the headset.
T.position(0.0, 1.12, -2.4).scale(1.8, 2.2, 0.03) {
    R.cube() { Mirror.quality(1024) {} }
}
T.position(-1.7, 2.35, -2.35).scale(0.04, 0.04, 1.0) {
    Text { "Flat-ground stepping: move, stop, strafe, crouch.\nSmall upright reach errors are expected in this slice." }
}
T.position(-3.4, 2.3, -1.5) {
    Grabbable {}
    xr_settings_panel({ root_name = "walking_xr_settings" })
}

T {
    InputXR.on() {
        InputXRGamepad { locomotion() speed(0.6) }
        T {
            AVC {
                initial_yaw(3.14159)
                crouch_sole_offsets([0.0, 0.0530325, 0.0874534], [0.0, 0.0530325, 0.0874534])
                WalkingIK {
                    stride(0.32)
                    step_duration(0.22)
                    step_height(0.07)
                    step_scale(1.3)
                }
                T {
                    GLTF.new("assets/models/bisket.glb") {
                        HandTracking.new().source("openxr")
                        bisket_humanoid_bone_map()
                        bisket_anime_shading()
                    }
                }
                T.position(0.0, 0.08, 0.12) { CXR { Pointer {} } }
                XRHand.new(true, "Left", "GripAim").laser() {
                    T {
                        RestAttachment.new("[name='J_Bip_L_Hand']", "[name='J_Bip_L_Middle3']") { Pointer {} }
                    }
                }
                XRHand.new(true, "Right", "GripAim").laser() {
                    T {
                        RestAttachment.new("[name='J_Bip_R_Hand']", "[name='J_Bip_R_Middle3']") { Pointer {} }
                    }
                }
            }
        }
    }
}
