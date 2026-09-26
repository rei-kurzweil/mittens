// Rei(mu) XR scene with skinned bow-ribbon secondary motion.
// Run: cargo run --release -- load 'examples/rei(mu).mms'
import { studio_stage } from "../assets/components/studio_stage.mms"
import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { bisket_humanoid_bone_map } from "../assets/components/humanoid_bone_maps/bisket.mms"
import { rei_mu_bow_secondary_motion } from "../assets/components/secondary_motion/rei-mu-bow.mms"
import { ambient_eye_saccades } from "../assets/components/animations/ambient_eye_saccades.mms"

// The default input stays neutral if no microphone is available.
let microphone = AudioInput {}
let voice_level = Amplitude.rolling_window(0.080).from(microphone) {}

BGC.rgba(0.19, 0.19, 0.21, 1.0)
AL.rgb(0.19, 0.19, 0.21)
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
                R.sphere() { C.rgba(0.34, 0.53, 0.30, 1.0) }
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

BG.occlusion_and_lighting() {
    star_kawaii_background([0.85, 0.82, 0.93, 1.0])
}

T.position(0.0, 7.0, 3.0) {
    DL { intensity(1.4) color(1.0, 0.96, 0.87) }
}

ED {
    studio_stage("rei_mu_first_stage")
    T.position(36.0, -1.5, -24.0).rotation(0.0, 1.5707963, 0.0) {
        studio_stage("rei_mu_second_stage")
    }

    tree_garden()

    T.position(-5.0, 2.55, 4.9).scale(1.5, 1.5, 0.08).rotation(0.0, 3.1416, 0.0) {
        name = "rei_mu_mirror"
        Grabbable {}
        R.cube() {
            Mirror.quality(1440) {}
            Raycastable.enabled()
        }
    }

    T.position(-5.0, 0.0, 0.0) {
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
                let rei_mu_avatar_control = AVC {
                    mouth_open_from_amplitude(voice_level)
                    mouth_open_rms_floor(0.005)
                    mouth_open_rms_ceiling(0.09)
                    mouth_open_smoothing(16.0)
                    voice_level

                    initial_yaw(3.14159)
                    left_arm_pole_direction([1, -0.35, 1])
                    right_arm_pole_direction([-1, -0.35, 1])
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

// Explicit panel selection suppresses the default world/assets/inspector UI.
// Keep settings off to the player's left so it does not cover the mirror.
T.position(-7.75, 2.8, -1.5) {
    name = "rei_mu_editor_ui"
    EditorUI { panels([{ panel = "settings" }]) }
}

XR.on()
