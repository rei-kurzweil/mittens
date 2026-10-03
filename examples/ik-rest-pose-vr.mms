// VR arm IK rest-pose comparison. The Rust launcher substitutes the model path
// and panel labels. Right controller B records each of the four hand positions.
// Run with --model bisket|rei --rest-pose t|a.
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { bisket_humanoid_bone_map } from "../assets/components/humanoid_bone_maps/bisket.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { rei_mu_bow_secondary_motion } from "../assets/components/secondary_motion/rei-mu-bow.mms"
import { ambient_eye_saccades } from "../assets/components/animations/ambient_eye_saccades.mms"

// The microphone stays neutral if no input device is available. These are
// the same mouth response settings used by the Rei(mu) XR scene.
let microphone = AudioInput {}
let voice_level = Amplitude.rolling_window(0.080)
    .highpass(120.0).highpass_resonance(0.707).from(microphone) {}
let mouth_tuning = { center_rms = 0.0475 range_rms = 0.085 amount = 1.0 }

RendererSettings.msaa_off() { window_size(800, 600) }
XR.on()
BGC.rgba(0.28, 0.32, 0.38, 1.0)
AL.rgb(0.62, 0.62, 0.65)
// The eye animation is authored in seconds at two beats per second.
Clock.bpm(120.0)
RenderGraph {
    EmissivePass {}
    Bloom { intensity(0.9) radius_ndc(0.045) emissive_scale(1.25) half_res(true) }
}

// Broad fill and two nearby key lights make the face, arms, and mirror
// reflection readable even with the anime material's shaded regions.
T.position(0.0, 2.5, 1.0) {
    DL { intensity(1.2) color(1.0, 0.96, 0.90) }
}
T.position(-1.2, 2.2, 0.7) {
    PL { intensity(6.0) distance(7.0) color(1.0, 0.92, 0.84) }
}
T.position(1.2, 1.5, -0.1) {
    PL { intensity(4.0) distance(6.0) color(0.75, 0.85, 1.0) }
}

T.position(0.0, -0.08, -0.6).scale(8.0, 0.08, 8.0) {
    R.cube() { C.rgba(0.48, 0.51, 0.54, 1.0) }
}

// The camera is inside the avatar's head. This mirror shows both arms while
// the tracked hands move. +Z is the mirror normal, toward the initial HMD.
T.position(0.0, 1.5, -2.36).scale(1.07, 1.35, 0.03) {
    name = "ik_rest_pose_mirror"
    Grabbable {}
    R.cube() { Mirror.quality(512) {} Raycastable.enabled() }
}

// Bisket and Rei(mu) use the same VRoid arm bone names.
T {
    InputXR.on() {
        InputXRGamepad { locomotion() speed(1.2) }
        T {
            let inspection_avatar = GLTF.new("MODEL_PATH") {
                bisket_anime_shading()
                bisket_humanoid_bone_map()
                MorphTargetMap.new()
                    .slot("left_eye_blink", "Fcl_EYE_Close_L")
                    .slot("right_eye_blink", "Fcl_EYE_Close_R")
                    .slot("viseme_aa", "Fcl_MTH_A")
                EM.on()
                bisket_colliders()
                bisket_shirt_physics(false)
                if MODEL_IS_REI {
                    // Includes ReturnToRestWhenStill on both bow ribbons.
                    rei_mu_bow_secondary_motion()
                }
            }
            let inspection_avatar_control = AVC {
                ik_debug()
                mouth_open_from_amplitude(voice_level)
                mouth_open_rms_center_range(mouth_tuning.center_rms, mouth_tuning.range_rms)
                mouth_open_amount(mouth_tuning.amount)
                mouth_open_smoothing(16.0)
                voice_level
                initial_yaw(3.14159)
                // Bias elbows down when hands are held close to the chest.
                left_arm_pole_direction([1, -1.5, 1])
                right_arm_pole_direction([-1, -1.5, 1])
                // Prototype exclusions observed while the left elbow turns inward.
                // The right arm remains unrestricted until we measure its bad angles.
                left_arm_forbidden_bend_normal_z_degrees(-178.0, -115.0)
                left_arm_forbidden_bend_normal_z_degrees(-100.0, -60.0)
                hand_rotation_smoothing(220.0)
                T {
                    inspection_avatar
                }
                T.position(0.0, 0.08, 0.12) {
                    name = "ik_readout_camera_anchor"
                    CXR { Pointer {} }
                }
                // Keep HTC eyelid closure for blinks; ambient animation owns
                // the eye-bone direction when pupil tracking is unreliable.
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
            inspection_avatar_control

            on(inspection_avatar, "GLTFInitialized", fn(event) {
                let left_eye = event.gltf.query("[name='J_Adj_L_FaceEye']")
                let right_eye = event.gltf.query("[name='J_Adj_R_FaceEye']")
                if left_eye && right_eye {
                    inspection_avatar_control.attach(ambient_eye_saccades(left_eye, right_eye, 2.0))
                } else {
                    print("GLTFInitialized: mapped eye bones were not found; ambient eye animation was not attached")
                }
            })
        }
    }
}

// The launcher updates these world-space positions and the angle labels each
// frame. MMS owns their camera-facing orientation. The camera marker receives
// the live XR camera position from the launcher before the next FrameTick.
let ik_readout_camera = T { name = "ik_readout_camera_world" }
fn bend_plane_readout(side, label) {
    return T {
        name = "ik_" + side + "_normal_mount"
        T {
            name = "ik_" + side + "_normal_panel"
            Grabbable {}
            T.position(0.0, 0.0, -0.008).scale(0.78, 0.20, 0.012) {
                R.cube() {
                    C.rgba(0.0, 0.0, 0.0, 1.0)
                    Raycastable.enabled()
                }
            }
            T.position(-0.35, 0.065, 0.014).scale(0.030, 0.030, 1.0) {
                Text { label name = "ik_" + side + "_normal_text" C.rgba(1.0, 1.0, 1.0, 1.0) }
            }
        }
    }
}
let ik_left_normal_mount = bend_plane_readout("left", "L raw Z: --\nIK Z: --")
let ik_right_normal_mount = bend_plane_readout("right", "R raw Z: --\nIK Z: --")
ik_readout_camera
ik_left_normal_mount
ik_right_normal_mount
let ik_left_normal_panel = ik_left_normal_mount.query("#ik_left_normal_panel")
let ik_right_normal_panel = ik_right_normal_mount.query("#ik_right_normal_panel")
on_global("FrameTick", fn(event) {
    let camera_world = ik_readout_camera.translation()
    ik_left_normal_panel.look_at(camera_world)
    ik_right_normal_panel.look_at(camera_world)
})

// Each capture row has one fixed origin. Both circles use that origin, so
// their centers cannot drift when labels wrap onto two lines.
fn capture_row(index, label, y) {
    return T.position(0.0, y, 0.0) {
        T.position(0.12, 0.0, 0.02).scale(0.105, 0.105, 1.0) {
            name = "capture_backing_" + index
            R.circle_2d() { C.rgba(0.24, 0.29, 0.30, 1.0) }
            T.position(0.0, 0.0, 0.01).scale(0.68, 0.68, 1.0) {
                name = "capture_lamp_" + index
                R.circle_2d() {
                    C.rgba(0.16, 1.0, 0.27, 1.0)
                    Emissive.off()
                }
            }
        }
        T.position(0.24, 0.018, 0.02).scale(0.044, 0.044, 1.0) {
            Text { label C.rgba(0.87, 0.95, 0.93, 1.0) TextureFiltering.linear() }
        }
    }
}

T.position(1.0, 1.78, -1.25) {
    name = "ik_capture_panel"
    T.position(0.54, -0.36, -0.015).scale(1.16, 0.88, 0.015) {
        R.cube() { C.rgba(0.025, 0.055, 0.065, 0.94) }
    }
    T.position(0.07, 0.02, 0.02).scale(0.036, 0.036, 1.0) {
        Text { "ARM IK  /  B TO CAPTURE" C.rgba(0.46, 0.94, 0.81, 1.0) }
    }
    T.position(0.07, -0.065, 0.02).scale(0.032, 0.032, 1.0) {
        Text { "MODEL_LABEL\nREST_POSE_LABEL" C.rgba(0.67, 0.83, 0.81, 1.0) }
    }
    capture_row("0", "Hands held\nout front", -0.22)
    capture_row("1", "Hands against\nchest", -0.37)
    capture_row("2", "Hands at\nlap / hips", -0.52)
    capture_row("3", "Hands at sides,\nslightly outward", -0.67)
}

// Legend belongs to this inspection scene. These colors match the runtime
// TwoBoneIK debug cubes; no engine-wide legend or color change is needed.
fn legend_row(y, color, label) {
    return T.position(0.0, y, 0.0) {
        T.position(0.10, 0.0, 0.02).scale(0.11, 0.018, 0.012) {
            R.cube() {
                C.rgba(color[0], color[1], color[2], 1.0)
                Emissive.on() { intensity(1.5) }
            }
        }
        T.position(0.20, 0.025, 0.02).scale(0.040, 0.040, 1.0) {
            Text { label C.rgba(0.92, 0.95, 0.96, 1.0) TextureFiltering.linear() }
        }
    }
}

T.position(-1.55, 1.78, -1.25) {
    name = "ik_debug_legend"
    T.position(0.47, -0.31, -0.015).scale(0.99, 0.75, 0.015) {
        R.cube() { C.rgba(0.025, 0.055, 0.065, 0.94) }
    }
    T.position(0.06, 0.02, 0.02).scale(0.039, 0.039, 1.0) {
        Text { "IK DEBUG LINES" C.rgba(0.46, 0.94, 0.81, 1.0) }
    }
    legend_row(-0.12, [1.0, 0.85, 0.15], "Shoulder to hand target")
    legend_row(-0.23, [0.10, 0.95, 1.0], "Elbow pole direction")
    legend_row(-0.34, [1.0, 0.35, 0.80], "Bend plane normal")
    legend_row(-0.45, [0.20, 1.0, 0.35], "Shoulder to solved elbow")
    legend_row(-0.56, [1.0, 1.0, 1.0], "Solved elbow point")
}
