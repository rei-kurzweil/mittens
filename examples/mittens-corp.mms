import { studio_stage, studio_floor } from "../assets/components/studio_stage.mms"
import { teleport_pit } from "../assets/components/teleport_pit.mms"
import { vroid_arm_ik } from "../assets/components/arm_ik/vroid.mms"

let arm_ik = vroid_arm_ik(-0.35, 1)

// mittens-corp — XR car/controller regression scene and Rei(mu) pose-authoring tool.
//
// Run with:
//   cargo run --release -- load examples/mittens-corp.mms
//
// The vehicle uses the generic inverse-local anchor operator rather than AVC:
// the rigid car has no humanoid specialization for AVC to perform.

import { tripod_light } from "../assets/components/tripod_light.mms"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_humanoid_bone_map } from "../assets/components/humanoid_bone_maps/bisket.mms"
import { rei_mu_bow_secondary_motion } from "../assets/components/secondary_motion/rei-mu-bow.mms"
import { ambient_eye_saccades } from "../assets/components/animations/ambient_eye_saccades.mms"
import { suspended_platform } from "../assets/components/platforms/suspended_platform.mms"
import { display_car_xr } from "../assets/components/vehicles/display_car.mms"

// Optional sources stay neutral when the runtime or hardware is unavailable.
let microphone = AudioInput {}
let voice_level = Amplitude.rolling_window(0.080).highpass(120.0).highpass_resonance(0.707).from(microphone) {}
let mouth_tuning = { center_rms = 0.0475 range_rms = 0.085 amount = 1.0 }

RendererSettings { window_size(1440, 810) }
BGC.rgba(0.055, 0.055, 0.060, 1.0)
AL.rgb(0.13, 0.13, 0.15)
// The default engine clock is 120 BPM, i.e. two beats per second. Keep this
// explicit because ambient_eye_saccades authors its keyframe times in seconds.
Clock.bpm(120.0)

RenderGraph {
    EmissivePass { BlurPass { radius_ndc(0.025) half_res(true) } }
    Bloom { intensity(0.42) radius_ndc(0.025) emissive_scale(1.0) half_res(true) }
}


// Keep the car example's studio stage so this tool also retains a stable
// lighting and mirror reference while XR transforms and gizmos are exercised.
studio_floor()

studio_stage("mittens_corp_stage")
teleport_pit("studio_teleport_pit", [0.0, -14.0, 0.0], [100.0, 12.0, 100.0], [-5.0, 1.2, 0.0], "spikes")

T.position(0.0, 2.55, 8.10).scale(1.5, 1.5, 0.08).rotation(0.0, 3.1416, 0.0) {
    name = "stage_mirror"
    Grabbable {}
    R.cube() {
        Mirror.quality(1440) {}
        Raycastable.enabled()
    }
}

// Three elevated walkway sections run along Z, perpendicular to the stage's
// long X axis. Their ends meet to form one continuous suspended platform.
for platform_index in range(3) {
    T.position(11.5, 4.0, (platform_index - 1) * 15.0) {
        suspended_platform()
    }
}

let subject_light_target = [0.0, 1.75, 1.7]
tripod_light(
    "front_left_studio_light",
    [-10.5, 0.14, 4.4],
    subject_light_target,
    SL.color(1.0, 0.82, 0.70).intensity(10.0).distance(22.0).angle(0.58).penumbra(0.32),
)
tripod_light(
    "front_right_studio_light",
    [10.5, 0.14, 4.4],
    subject_light_target,
    SL.color(0.72, 0.84, 1.0).intensity(10.0).distance(22.0).angle(0.58).penumbra(0.32),
)
tripod_light(
    "rear_left_studio_light",
    [-10.5, 0.14, -5.7],
    subject_light_target,
    SL.color(0.72, 0.84, 1.0).intensity(8.0).distance(20.0).angle(0.62).penumbra(0.38),
)
tripod_light(
    "rear_right_studio_light",
    [10.5, 0.14, -5.7],
    subject_light_target,
    SL.color(1.0, 0.78, 0.68).intensity(8.0).distance(20.0).angle(0.62).penumbra(0.38),
)

// Rei(mu) is the player rig. InputXR continues to own tracked head translation
// and rotation; only the gamepad's built-in locomotion mapping is handed off
// when a vehicle layer takes movement authority.
ED.active() {
    let vehicle_controls = InputXRGamepad {
        name = "rei_mu_pedestrian_locomotion"
        locomotion()
        speed(1.5)
    }

    let player_motion = Velocity {
        name = "rei_mu_locomotion_root_velocity"
        T.position(-5.0, 1.0, 0.0) {
            name = "rei_mu_locomotion_root"
            Rider
                .anchor("[name='rei_mu_rider_cxr_anchor']")
                .movement_root("[name='rei_mu_locomotion_root']")
                .input("[name='rei_mu_pedestrian_locomotion']") {}
            InputXR.on() {
                vehicle_controls

                T {
                    name = "rei_mu_xr_driver"
                    let rei_mu_avatar = GLTF.new("assets/models/rei(mu).glb") {
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
                        // Includes ReturnToRestWhenStill on both bow ribbons.
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

                        // Rider-side anchor. AVC reparents this wrapper beneath the
                        // head; mounting aligns it with the car's cockpit target.
                        T.position(0.0, 0.08, 0.12) {
                            name = "rei_mu_rider_cxr_anchor"
                            CXR { Pointer {} }
                        }
                        // HTC or VRChat OSC supplies closure samples for blink morphs,
                        // while authored animation owns the eye-bone direction.
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

                    // The shared VRoid humanoid map above declares these two
                    // skin-joint targets. Query only this GLTF instance after it
                    // finishes importing, so another avatar cannot be animated.
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
        if event.control == "ButtonY" && player_motion.grounded() { player_motion.translate_world([0.0, 4.5, 0.0]) }
    })

    let car_root = display_car_xr(
        "left_display_car",
        [-19.0, -0.75, -1.5],
        0.30,
        "left_display_car_cxr_mount",
        vehicle_controls,
    )
    car_root
}

// Explicit selection disables every editor window except the two needed for
// pose-authoring and transform/gizmo diagnostics.
T.position(1.25, 2.8, -1.5) {
    name = "mittens_corp_editor_ui"
    EditorUI {
        panels([
            { panel = "settings" config = { show_zones = true } },
            { panel = "pose" },
        ])
    }
}

// Falling starts only after AVC's generated slide capsule is usable.
let gravity_avatar = query("[name='rei_mu_avatar_control']")
on(gravity_avatar, "DataEvent", fn(event) {
    if event == "CapsuleReady" {
        query("[name='player_gravity']").set_enabled(true)
    }
})
// Also support registering this policy after the readiness transition.
if gravity_avatar.capsule_ready() {
    query("[name='player_gravity']").set_enabled(true)
}

XR.on()
