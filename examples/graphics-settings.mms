// XR graphics settings example: mirror capture/resolution and secondary motion.
// Run: cargo run --release -- load examples/graphics-settings.mms
import { graphics_settings_panel } from "../assets/components/panels/graphics_settings_panel.mms"
import { stage_with_boxes } from "../assets/components/stage_with_boxes.mms"
import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"
import { teleport_pit } from "../assets/components/teleport_pit.mms"
import { vroid_arm_ik_bend_plane_prototype } from "../assets/components/arm_ik/vroid.mms"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { bisket_humanoid_bone_map } from "../assets/components/humanoid_bone_maps/bisket.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { rei_mu_bow_secondary_motion } from "../assets/components/secondary_motion/rei-mu-bow.mms"
import { ambient_eye_saccades } from "../assets/components/animations/ambient_eye_saccades.mms"

RendererSettings { window_size(1280, 720) }
BGC.rgba(0.0, 0.0, 0.0, 1.0)
AL.rgb(0.24, 0.27, 0.38)
Clock.bpm(120.0)
RenderGraph {
    EmissivePass { BlurPass { radius_ndc(0.045) half_res(true) } }
    Bloom { intensity(0.8) radius_ndc(0.045) emissive_scale(1.25) half_res(true) }
}
BG.occlusion_and_lighting() {
    star_kawaii_background([1.0, 0.82, 0.12, 1.0])
}

stage_with_boxes("graphics_settings_stage")
teleport_pit("graphics_settings_teleport_pit", [0.0, -12.0, 0.0], [120.0, 12.0, 120.0], [0.0, 1.2, 1.0], "spikes")

// Full-body reflection next to the standalone settings panel. The reflective
// +Z face points toward the player's starting position; grab to reposition it.
T.position(2.5, 1.3, -1.0).scale(1.8, 2.6, 0.08) {
    name = "graphics_settings_mirror"
    Grabbable {}
    R.cube() {
        Mirror.quality(512) {}
        Raycastable.enabled()
    }
}

// The factory owns its layout, drag handle, and accordion restore. This scene
// places the panel in front of the player's starting position.
T.position(-1.28, 2.3, -1.0) {
    graphics_settings_panel({
        root_name = "graphics_settings"
        title = T {
            name = "graphics_settings_title_bar"
            Grabbable.parent()
            Draggable.target("../../#graphics_settings")
            Raycastable.enabled()
            Style {
                display("flex") flex_direction("row") align_items("center")
                width(100%) height(3.5)
                background_color([0.16, 0.14, 0.09, 0.98]) background_z(-0.01)
                color([0.96, 0.93, 0.82, 1.0])
            }
            T {
                Style {
                    display("flex") width(0.0) flex_grow(1.0) height(3.5)
                    align_items("center") padding_xy(0.9, 0.3)
                }
                T.position(0.0, 0.0, 0.02) { Text { "Graphics settings" } }
            }
        }
    })
}

// Keep the editor workspace limited to these three panels. The graphics
// settings preview above is an independent authored scene panel.
T.position(-4.5, 2.8, -1.0) {
    name = "graphics_settings_editor_ui"
    EditorUI {
        panels([
            { panel = "settings" },
            { panel = "paint" },
            { panel = "color" },
        ])
    }
}

// Match mittens-corp's microphone response and Rei(mu) player features.
let microphone = AudioInput {}
let voice_level = Amplitude.rolling_window(0.080).highpass(120.0).highpass_resonance(0.707).from(microphone) {}
let mouth_tuning = { center_rms = 0.0475 range_rms = 0.085 amount = 1.0 }

ED.active() {
    let player_motion = Velocity {
        name = "rei_mu_locomotion_root_velocity"
        T.position(0.0, 1.0, 1.0) {
            name = "rei_mu_locomotion_root"
            InputXR.on() {
                InputXRGamepad {
                    name = "rei_mu_pedestrian_locomotion"
                    locomotion()
                    speed(1.5)
                }
                T {
                    name = "rei_mu_xr_driver"
                    let rei_mu_avatar = GLTF.new("assets/models/rei(mu).glb") {
                        HandTracking.new().source("openxr")
                        bisket_anime_shading()
                        bisket_humanoid_bone_map()
                        MorphTargetMap.new()
                            .slot("left_eye_blink", "Fcl_EYE_Close_L")
                            .slot("right_eye_blink", "Fcl_EYE_Close_R")
                            .slot("viseme_aa", "Fcl_MTH_A")
                        EM.on()
                        PoseCapture { label("Rei(mu)") asset_name("rei_mu") }
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
                        arm_ik(vroid_arm_ik_bend_plane_prototype())
                        hand_rotation_smoothing(220.0)
                        T { rei_mu_avatar }

                        T.position(0.0, 0.08, 0.12) {
                            name = "rei_mu_head_camera_anchor"
                            CXR { Pointer {} }
                        }
                        // Keep independent eyelid samples from XR/ALVR or OSC;
                        // authored ambient animation owns pupil direction.
                        XREyeTracking.on().priority(["htc", "vrchat_osc"]).enable_pupil_direction_tracking(false)
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
                            print("GLTFInitialized: Rei(mu) mapped eye bones were not found; ambient eye animation was not attached")
                        }
                    })
                }
            }
        }
    }
    Gravity.enabled(false) { name = "player_gravity" player_motion }
    on_global("XrButtonDown", fn(event) {
        if event.control == "ButtonB" && player_motion.grounded() {
            player_motion.translate_world([0.0, 4.5, 0.0])
        }
    })
}

// Enable falling only when AVC's generated slide capsule is ready.
let gravity_avatar = query("[name='rei_mu_avatar_control']")
on(gravity_avatar, "DataEvent", fn(event) {
    if event == "CapsuleReady" { query("[name='player_gravity']").set_enabled(true) }
})
if gravity_avatar.capsule_ready() { query("[name='player_gravity']").set_enabled(true) }

XR.on()
