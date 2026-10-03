import { vroid_arm_ik } from "../assets/components/arm_ik/vroid.mms"

let arm_ik = vroid_arm_ik(-0.35, -1)

// VTuber eye-tracking mirror
//
// A minimal XR avatar scene for validating AVC automatic eye-bone tracking.
// `HTCEyeTracking` must remain a *direct child* of `AVC`: this focused scene
// explicitly validates the HTC binary transport and its retained gaze state.
//
// The mirror is in front of the XR start pose. The bisket asset's humanoid map
// supplies the eye slots automatically; an avatar without mapped eye slots is
// left untouched.

import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"
import { tripod_light } from "../assets/components/tripod_light.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"

RendererSettings { window_size(960, 720) }
BGC.rgba(0.0, 0.0, 0.0, 1.0)
AL.rgb(0.18, 0.18, 0.22)

RenderGraph {
    EmissivePass { BlurPass { radius_ndc(0.06) half_res(true) } }
    Bloom { intensity(0.8) emissive_scale(1.1) }
}

BG.occlusion_and_lighting() {
    star_kawaii_background([1.0, 0.78, 0.35, 1.0])
}

// Three physical studio fixtures make the mirror's lighting easy to inspect.
tripod_light("eye_tracking_key", [-4.2, 0.0, 2.8], [0.0, 1.25, -1.5], SL.color(1.0, 0.78, 0.62).intensity(6.0).distance(11.0).angle(0.62).penumbra(0.35))
tripod_light("eye_tracking_fill", [4.0, 0.0, 1.4], [0.0, 1.25, -1.5], SL.color(0.48, 0.68, 1.0).intensity(4.5).distance(11.0).angle(0.62).penumbra(0.35))
tripod_light("eye_tracking_rim", [1.8, 0.0, -4.2], [0.0, 1.25, -1.5], SL.color(1.0, 0.42, 0.78).intensity(5.0).distance(11.0).angle(0.62).penumbra(0.35))

// Microphone mouth-open test. Evaluate AudioInput.devices() in the REPL to
// see the numbered input list. Use AudioInput {} for the OS default, or replace
// it with AudioInput.device_number(0) / (1) to test enumerated input devices.
// The console prints the selected device/format and one RMS diagnostic per
// second; use those values to tune the floor and ceiling below.
let microphone = AudioInput {}
let voice_level = Amplitude.rolling_window(0.080).from(microphone) {}

// An explicit list keeps the editor useful without materializing the default
// world and assets panels in this focused capture scene.
T.position(-2.75, 2.8, -1.5) {
    EditorUI {
        panels([{
            panel = "settings"
            config = {
                show_armature = false
                show_bounds = false
                show_cameras = false
                show_colliders = false
                show_gltf_colliders = false
                show_spring_bones = false
            }
        }])
    }
}

ED {
    // Low presentation stage, with colored block piles marking its edges.
    T.position(0.0, -0.85, -1.5).scale(9.8, 0.16, 8.4) {
        R.cube() { C.rgba(0.20, 0.22, 0.27, 1.0) }
    }

    // Left pile.
    T.position(-7.70, -0.56, -0.7).scale(0.34, 0.34, 0.34) { Grabbable {} R.cube() { C.rgba(1.0, 0.38, 0.56, 1.0) EM.on() } }
    T.position(-7.70, -0.22, -0.7).scale(0.34, 0.34, 0.34) { Grabbable {} R.cube() { C.rgba(1.0, 0.72, 0.20, 1.0) EM.on() } }
    T.position(-7.35, -0.56, -0.7).scale(0.34, 0.34, 0.34) { Grabbable {} R.cube() { C.rgba(0.40, 0.76, 1.0, 1.0) EM.on() } }

    // Right pile.
    T.position(7.70, -0.56, -0.7).scale(0.34, 0.34, 0.34) { Grabbable {} R.cube() { C.rgba(0.44, 0.92, 0.66, 1.0) EM.on() } }
    T.position(7.70, -0.22, -0.7).scale(0.34, 0.34, 0.34) { Grabbable {} R.cube() { C.rgba(0.54, 0.48, 1.0, 1.0) EM.on() } }
    T.position(7.35, -0.56, -0.7).scale(0.34, 0.34, 0.34) { Grabbable {} R.cube() { C.rgba(1.0, 0.48, 0.82, 1.0) EM.on() } }

    // Rear pile.
    T.position(0.0, -0.56, 5.85).scale(0.34, 0.34, 0.34) { Grabbable {} R.cube() { C.rgba(0.25, 0.78, 1.0, 1.0) EM.on() } }
    T.position(0.0, -0.22, 5.85).scale(0.34, 0.34, 0.34) { Grabbable {} R.cube() { C.rgba(0.98, 0.90, 0.30, 1.0) EM.on() } }
    T.position(-0.35, -0.56, 5.85).scale(0.34, 0.34, 0.34) { Grabbable {} R.cube() { C.rgba(1.0, 0.48, 0.42, 1.0) EM.on() } }

    // Deliberate versions of the friendly fallback-scene accents: a color cat
    // and a rainbow row of planes behind the stage.
    T.position(0.0, -0.10, -4.0).scale(0.50, 0.50, 0.50) {
        Grabbable {}
        GLTF.new("assets/models/color-cat.2.glb") {}
    }
    T.position(-1.40, 1.95, 1.75).scale(0.28, 0.28, 1.0) { R.plane() { C.rgba(1.0, 0.20, 0.20, 1.0) EM.on() } }
    T.position(-0.84, 1.95, 1.75).scale(0.28, 0.28, 1.0) { R.plane() { C.rgba(1.0, 0.60, 0.20, 1.0) EM.on() } }
    T.position(-0.28, 1.95, 1.75).scale(0.28, 0.28, 1.0) { R.plane() { C.rgba(1.0, 1.0, 0.20, 1.0) EM.on() } }
    T.position(0.28, 1.95, 1.75).scale(0.28, 0.28, 1.0) { R.plane() { C.rgba(0.20, 0.80, 0.35, 1.0) EM.on() } }
    T.position(0.84, 1.95, 1.75).scale(0.28, 0.28, 1.0) { R.plane() { C.rgba(0.20, 0.60, 1.0, 1.0) EM.on() } }
    T.position(1.40, 1.95, 1.75).scale(0.28, 0.28, 1.0) { R.plane() { C.rgba(0.80, 0.20, 1.0, 1.0) EM.on() } }

    // Full-body mirror facing the avatar.
    T.position(0.0, 1.25, -4.5).scale(2.4, 2.4, 0.08) {
        R.cube() { Mirror.quality(2048) {} }
    }

    T {
        InputXR.on() {
            InputXRGamepad { locomotion() speed(1.5) }
            T {
                AVC {
                    mouth_open_from_amplitude(voice_level)
                    mouth_open_rms_floor(0.005)
                    mouth_open_rms_ceiling(0.09)
                    mouth_open_smoothing(16.0)

                    // Direct placement also makes the retained measurement
                    // available to AVC diagnostics. Capture is not monitored.
                    voice_level

                    // OpenXR's rest-forward is -Z.
                    initial_yaw(3.14159)
                    left_two_bone_ik(arm_ik.left)
                    right_two_bone_ik(arm_ik.right)
                    hand_rotation_smoothing(220.0)
                    // head_motion_gaze_policy("freeze")

                    T {
                        GLTF.new("assets/models/bisket.glb") {
                            bisket_anime_shading()
                            // Bisket's VRM-style face mesh exposes these exact
                            // targetNames on each of its eight material
                            // primitives. Generic OSC closure fans each
                            // semantic channel out to every matching target.
                            MorphTargetMap.new()
                                .slot("left_eye_blink", "Fcl_EYE_Close_L")
                                .slot("right_eye_blink", "Fcl_EYE_Close_R")
                                // The same avatar is the first microphone-viseme
                                // validation target. These canonical keys are inert
                                // until a direct Visemes child is added below AVC.
                                .slot("viseme_aa", "Fcl_MTH_A")
                                .slot("viseme_ih", "Fcl_MTH_I")
                                .slot("viseme_ou", "Fcl_MTH_U")
                                .slot("viseme_e", "Fcl_MTH_E")
                                .slot("viseme_oh", "Fcl_MTH_O")
                            EM.on()
                            bisket_colliders()
                            bisket_shirt_physics(false)
                        }
                    }

                    // AVC reparents this camera path to the mapped head.
                    T.position(0.0, 0.08, 0.12) {
                        CXR { Pointer {} }
                    }

                    // Direct child: enables automatic mapped left/right eye
                    // bone rotation. AVC freezes retained gaze during rapid
                    // head motion; the tracker caps head-local gaze first.
                    HTCEyeTracking.on().rotation_limits(0.35, 0.35, 0.25, 0.25)

                    // Direct children supply the left/right hand targets used
                    // by AVC's mapped TwoBoneIK chains.
                    XRHand.new(true, "Left", "GripAim").laser() {
                        T {
                            RestAttachment.new("[name='J_Bip_L_Hand']", "[name='J_Bip_L_Middle3']") {
                                Pointer {}
                            }
                        }
                    }
                    XRHand.new(true, "Right", "GripAim").laser() {
                        T {
                            RestAttachment.new("[name='J_Bip_R_Hand']", "[name='J_Bip_R_Middle3']") {
                                Pointer {}
                            }
                        }
                    }
                }
            }
        }
    }
}

XR.on()
