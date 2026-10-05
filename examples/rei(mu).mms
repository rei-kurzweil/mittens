import { teleport_pit } from "../assets/components/teleport_pit.mms"
import { vroid_arm_ik } from "../assets/components/arm_ik/vroid.mms"

let arm_ik = vroid_arm_ik(-0.35, 1)

// Rei(mu) XR scene with skinned bow-ribbon secondary motion.
// Run: cargo run --release -- load 'examples/rei(mu).mms'
import { studio_stage } from "../assets/components/studio_stage.mms"
import { tripod_light } from "../assets/components/tripod_light.mms"
import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { bisket_humanoid_bone_map } from "../assets/components/humanoid_bone_maps/bisket.mms"
import { rei_mu_bow_secondary_motion } from "../assets/components/secondary_motion/rei-mu-bow.mms"
import { ambient_eye_saccades } from "../assets/components/animations/ambient_eye_saccades.mms"
import { mouth_response_panel } from "../assets/components/ui/mouth_response_panel.mms"

// The default input stays neutral if no microphone is available.
let microphone = AudioInput {}
let voice_level = Amplitude.rolling_window(0.080).highpass(120.0).highpass_resonance(0.707).from(microphone) {}
// Preserve this scene's existing raw-amplitude calibration as the panel default.
let mouth_tuning = { center_rms = 0.0475 range_rms = 0.085 amount = 1.0 }
let mouth_panel_target = { avatar = null }

BGC.rgba(0.12, 0.12, 0.12, 1.0)
AL.rgb(0.24, 0.23, 0.25)
// The eye animation is authored in seconds and runs at two beats per second.
Clock.bpm(120.0)

RenderGraph {
    EmissivePass { BlurPass { radius_ndc(0.025) half_res(true) } }
    Bloom { intensity(0.35) radius_ndc(0.025) emissive_scale(1.0) half_res(true) }
}

fn garden_box(box_name, x, y, z, width, height, depth, color) {
    return T.position(x, y, z).scale(width, height, depth) {
        name = box_name
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) }
    }
}

// Each level places a branch and recursively grows three smaller branches.
// Four levels keep the tree compact enough for a live XR scene.
fn tree_branch(depth, length, width) {
    return T {
        T.position(0.0, length * 0.5, 0.0).scale(width, length, width) {
            R.cube() { C.rgba(0.34, 0.21, 0.12, 1.0) }
        }
        if depth > 0 {
            T.position(0.0, length, 0.0).rotation(0.48, 0.0, 0.43) {
                tree_branch(depth - 1, length * 0.68, width * 0.70)
            }
            T.position(0.0, length, 0.0).rotation(-0.40, 2.10, -0.46) {
                tree_branch(depth - 1, length * 0.68, width * 0.70)
            }
            T.position(0.0, length, 0.0).rotation(0.38, -2.10, 0.34) {
                tree_branch(depth - 1, length * 0.68, width * 0.70)
            }
        } else {
            T.position(0.0, length, 0.0).scale(0.26, 0.26, 0.26) {
                R.sphere() { C.rgba(0.96, 0.75, 0.83, 1.0) }
            }
        }
    }
}

fn tree_garden() {
    return T.position(6.0, 0.0, -1.3) {
        name = "tree_garden"
        garden_box("garden_soil", 0.0, 0.20, 0.0, 4.2, 0.16, 4.2, [0.52, 0.40, 0.30])
        garden_box("garden_north_border", 0.0, 0.29, -2.10, 4.5, 0.34, 0.16, [0.64, 0.61, 0.55])
        garden_box("garden_south_border", 0.0, 0.29,  2.10, 4.5, 0.34, 0.16, [0.64, 0.61, 0.55])
        garden_box("garden_east_border", 2.10, 0.29, 0.0, 0.16, 0.34, 4.2, [0.64, 0.61, 0.55])
        garden_box("garden_west_border", -2.10, 0.29, 0.0, 0.16, 0.34, 4.2, [0.64, 0.61, 0.55])
        T.position(0.0, 0.28, 0.0) {
            name = "garden_tree"
            tree_branch(3, 1.15, 0.24)
        }
    }
}

fn lit_stage(stage_name, left_light_name, right_light_name, light_target) {
    return T {
        studio_stage(stage_name)
        tripod_light(
            left_light_name,
            [-10.5, 0.14, 4.4],
            light_target,
            SL.color(1.0, 0.83, 0.74).intensity(9.0).distance(22.0).angle(0.65).penumbra(0.38),
        )
        tripod_light(
            right_light_name,
            [10.5, 0.14, 4.4],
            light_target,
            SL.color(0.78, 0.86, 1.0).intensity(9.0).distance(22.0).angle(0.65).penumbra(0.38),
        )
    }
}

BG.occlusion_and_lighting() {
    star_kawaii_background([1.0, 0.68, 0.1, 1.0])
}

lit_stage(
    "rei_mu_first_stage", "rei_mu_first_left_light", "rei_mu_first_right_light",
    [0.0, 1.75, -1.5],
)
// The near edge of this stage meets the spawn stage's right edge.
T.position(28.0, 0.0, -8.0).rotation(0.0, 0.7853982, 0.0) {
    lit_stage(
        "rei_mu_second_stage", "rei_mu_second_left_light", "rei_mu_second_right_light",
        [28.0, 1.75, -8.0],
    )
}
// A lower stage mirrors the bend on the opposite side of the spawn stage.
T.position(-28.0, -10.0, -8.0).rotation(0.0, -0.7853982, 0.0) {
    lit_stage(
        "rei_mu_lower_stage", "rei_mu_lower_left_light", "rei_mu_lower_right_light",
        [-28.0, -8.25, -8.0],
    )
}

tree_garden()

// A world-space performer control beside the stage. The title remains
// draggable; physical grab-to-shrink is tracked separately.
let rei_mu_mouth_panel = mouth_response_panel({
    root_name = "rei_mu_mouth_response_panel"
    title = "Voice response"
    avatar_slot = mouth_panel_target
    filter_slot = { amplitude = voice_level cutoff_hz = 120.0 resonance = 0.707 }
    tuning = mouth_tuning
    description = "Map raw microphone amplitude to mouth movement. These controls do not change audio input or volume."
})
T.position(-1.30, 2.05, 1.35) {
    name = "rei_mu_mouth_panel_anchor"
    rei_mu_mouth_panel
}

T.position(-5.0, 2.55, 4.9).scale(1.5, 1.5, 0.08).rotation(0.0, 3.1416, 0.0) {
    name = "rei_mu_mirror"
    Grabbable {}
    R.cube() {
        Mirror.quality(1440) {}
        Raycastable.enabled()
    }
}

let player_motion = Velocity {
    name = "rei_mu_velocity"
    T.position(-5.0, 1.0, 0.0) {
        name = "rei_mu_locomotion_root"
        InputXR.on() {
            InputXRGamepad { locomotion() speed(1.5) }
            T {
                let rei_mu_avatar = GLTF.new("assets/models/rei(mu).glb") {
                    bisket_anime_shading()
                    bisket_humanoid_bone_map()
                    MorphTargetMap.new()
                        .slot("left_eye_blink", "Fcl_EYE_Close_L")
                        .slot("right_eye_blink", "Fcl_EYE_Close_R")
                        .slot("viseme_aa", "Fcl_MTH_A")
                    EM.on()
                    bisket_colliders()
                    bisket_shirt_physics(false)
                    rei_mu_bow_secondary_motion()
                }
                let rei_mu_avatar_control = AVC.movement_target("[name='rei_mu_locomotion_root']") {
                    name = "rei_mu_avatar_control"
                    mouth_open_from_amplitude(voice_level)
                    mouth_open_rms_center_range(mouth_tuning.center_rms, mouth_tuning.range_rms)
                    mouth_open_amount(mouth_tuning.amount)
                    mouth_open_smoothing(16.0)
                    voice_level

                    initial_yaw(3.14159)
                    left_two_bone_ik(arm_ik.left)
                    right_two_bone_ik(arm_ik.right)
                    hand_rotation_smoothing(220.0)

                    T {
                        rei_mu_avatar
                    }

                    T.position(0.0, 0.08, 0.12) {
                        name = "rei_mu_xr_camera"
                        CXR { Pointer {} }
                    }
                    // HTC closure can still drive blink; the idle animation
                    // owns pupil direction instead of live gaze samples.
                    HTCEyeTracking.on().enable_pupil_direction_tracking(false)

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
                mouth_panel_target.avatar = rei_mu_avatar_control
                rei_mu_avatar_control

                on(rei_mu_avatar, "GLTFInitialized", fn(event) {
                    let left_eye = event.gltf.query("[name='J_Adj_L_FaceEye']")
                    let right_eye = event.gltf.query("[name='J_Adj_R_FaceEye']")
                    if left_eye && right_eye {
                        rei_mu_avatar_control.attach(ambient_eye_saccades(left_eye, right_eye, 2.0))
                    } else {
                        print("GLTFInitialized: Rei(mu) eye bones were not found; idle pupil animation was not attached")
                    }
                })
            }
        }
    }
}
Gravity { player_motion }
on_global("XrButtonDown", fn(event) {
    if event.control == "ButtonY" && player_motion.grounded() { player_motion.translate_world([0.0, 4.5, 0.0]) }
})
teleport_pit("rei_mu_teleport_pit", [0.0, -24.0, 0.0], [180.0, 12.0, 180.0], [-5.0, 1.2, 0.0], "none")

fn pile_box(box_name, x, y, z, color) {
    return T.position(x, y, z).scale(1.0, 1.0, 1.0) {
        name = box_name
        Grabbable {}
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) }
    }
}

// Only the two small box piles are part of the editable scene.
ED {
    // Three boxes below, two above.
    pile_box("left_pile_red_base",   -13.0, 0.62, -2.8, [0.95, 0.62, 0.65])
    pile_box("left_pile_green_base", -12.0, 0.62, -2.8, [0.66, 0.90, 0.68])
    pile_box("left_pile_blue_base",  -11.0, 0.62, -2.8, [0.67, 0.80, 0.97])
    pile_box("left_pile_blue_top",   -12.5, 1.62, -2.8, [0.67, 0.80, 0.97])
    pile_box("left_pile_red_top",    -11.5, 1.62, -2.8, [0.95, 0.62, 0.65])

    // Two boxes below, one above.
    pile_box("right_pile_green_base", -12.0, 0.62, 1.0, [0.66, 0.90, 0.68])
    pile_box("right_pile_blue_base",  -11.0, 0.62, 1.0, [0.67, 0.80, 0.97])
    pile_box("right_pile_red_top",    -11.5, 1.62, 1.0, [0.95, 0.62, 0.65])
}

// Explicit panel selection suppresses the default world/assets/inspector UI.
// Keep settings off to the player's left so it does not cover the mirror.
T.position(-7.75, 2.8, -1.5) {
    name = "rei_mu_editor_ui"
    EditorUI { panels([{ panel = "settings" }]) }
}

XR.on()
