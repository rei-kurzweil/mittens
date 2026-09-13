// mittens-corp — XR car/controller regression scene and Bisket pose-authoring tool.
//
// Run with:
//   cargo run --release -- load examples/mittens-corp.mms
//
// The vehicle uses the generic inverse-local anchor operator rather than AVC:
// the rigid car has no humanoid specialization for AVC to perform.

import { tripod_light } from "../assets/components/tripod_light.mms"
import { truss } from "../assets/components/truss.mms"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_humanoid_bone_map } from "../assets/components/humanoid_bone_maps/bisket.mms"
import { ambient_eye_saccades } from "../assets/components/animations/ambient_eye_saccades.mms"
import { suspended_platform } from "../assets/components/platforms/suspended_platform.mms"
import { display_car } from "../assets/components/vehicles/display_car.mms"

// Optional sources stay neutral when the runtime or hardware is unavailable.
let microphone = AudioInput {}
let voice_level = Amplitude.rolling_window(0.080).from(microphone) {}

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

fn stage_box(box_name, position, size, color) {
    return T.position(position[0], position[1], position[2])
        .scale(size[0], size[1], size[2]) {
        name = box_name
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) }
    }
}

// Keep the car example's studio stage so this tool also retains a stable
// lighting and mirror reference while XR transforms and gizmos are exercised.
stage_box(
    "studio_floor",
    [0.0, -0.92, 1.0],
    [54.0, 0.14, 32.0],
    [0.035, 0.037, 0.043],
)

stage_box("stage_deck",       [0.0,  0.00, -1.5], [32.0, 0.24, 14.0], [0.18, 0.18, 0.20])
stage_box("stage_upper_step", [0.0, -0.24,  5.7], [32.0, 0.28,  0.8], [0.14, 0.14, 0.16])
stage_box("stage_lower_step", [0.0, -0.56,  6.3], [32.0, 0.36,  0.8], [0.10, 0.10, 0.12])
stage_box("stage_back_wall",  [0.0,  4.00, -8.35], [32.0, 8.00, 0.35], [0.105, 0.105, 0.12])

T.position(0.0, 2.55, 8.10).scale(1.5, 1.5, 0.08).rotation(0.0, 3.1416, 0.0) {
    name = "stage_mirror"
    Grabbable {}
    R.cube() {
        Mirror.quality(1440) {}
        Raycastable.enabled()
    }
}

T.position(0.0, 7.10, -7.75) {
    name = "stage_ceiling_truss"
    truss(26)
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

// Bisket is the player rig. InputXR continues to own tracked head translation
// and rotation; only the gamepad's built-in locomotion mapping is handed off
// when a vehicle layer takes movement authority.
ED.active() {
    let vehicle_controls = InputXRGamepad {
        name = "bisket_pedestrian_locomotion"
        locomotion()
        speed(1.5)
    }
    // MMS tables are heap-backed, so every deferred handler below observes the
    // same mutable vehicle state rather than its own captured scalar snapshot.
    let vehicle_state = {
        mounted = false
        left_stick = [0.0, 0.0]
        right_grip_held = false
        position = [-19.0, -0.75, -1.5]
        yaw = 0.30
    }
    let car_drive_speed = 5.0
    let car_turn_speed = 1.25
    let car_stick_deadzone = 0.16

    T.position(-5.0, 0.0, 0.0) {
        name = "bisket_locomotion_root"
        Rider
            .anchor("[name='bisket_rider_cxr_anchor']")
            .movement_root("[name='bisket_locomotion_root']")
            .input("[name='bisket_pedestrian_locomotion']") {}
        InputXR.on() {
            vehicle_controls

            T {
                name = "bisket_xr_driver"
                let bisket_avatar = GLTF.new("assets/models/bisket.glb") {
                    bisket_anime_shading()
                    bisket_humanoid_bone_map()
                    MorphTargetMap.new()
                        .slot("left_eye_blink", "Fcl_EYE_Close_L")
                        .slot("right_eye_blink", "Fcl_EYE_Close_R")
                        .slot("viseme_aa", "Fcl_MTH_A")
                    EM.on()
                    PoseCapture { label("Bisket") asset_name("bisket") }
                    bisket_colliders()
                    bisket_shirt_physics(false)
                }
                let bisket_avatar_control = AVC {
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
                        bisket_avatar
                    }

                    // Rider-side anchor. AVC reparents this wrapper beneath the
                    // head; mounting aligns it with the car's cockpit target.
                    T.position(0.0, 0.08, 0.12) {
                        name = "bisket_rider_cxr_anchor"
                        CXR { Pointer {} }
                    }
                    // HTC eye tracking retains closure samples for blink morphs,
                    // while authored animation owns the eye-bone direction.
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
                bisket_avatar_control

                // The explicit Bisket humanoid map above declares these two
                // skin-joint targets. Query only this GLTF instance after it
                // finishes importing, so another avatar cannot be animated.
                on(bisket_avatar, "GLTFInitialized", fn(event) {
                    let left_eye = event.gltf.query("[name='J_Adj_L_FaceEye']")
                    let right_eye = event.gltf.query("[name='J_Adj_R_FaceEye']")
                    if left_eye && right_eye {
                        bisket_avatar_control.attach(ambient_eye_saccades(left_eye, right_eye, 2.0))
                    } else {
                        print("GLTFInitialized: Bisket mapped eye bones were not found; ambient eye animation was not attached")
                    }
                })
            }
        }
    }

    // Measure once after import; animate within a positioned muzzle frame so
    // keyframe closures do not need to capture late-loaded placement values.
    let laser_placement = { ready = false }
    // `R.square()` is a unit XY quad. After the beam root's X rotation, the
    // child Y scale is therefore the complete visible beam length, not its
    // half-length. Keep the center one half-length down local -Z so its near
    // edge stays exactly at the muzzle.
    let laser_length = 40.0
    let muzzle_clearance = 0.10
    let muzzle_height_fraction = 0.56

    let muzzle_flash_0_emissive = Emissive.off()
    let muzzle_flash_1_emissive = Emissive.off()
    let laser_outer_emissive = Emissive.off()
    let laser_middle_emissive = Emissive.off()
    let laser_core_emissive = Emissive.off()

    let muzzle_flash_0 = T.position(0.0, 0.0, 0.0).scale(0.0, 0.0, 0.0) {
        name = "car_laser_muzzle_flash_0"
        R.square() {
            C.rgba(1.0, 1.0, 1.0, 1.0)
            Texture.with_uri("assets/images/flash_red_0.png")
            TextureFiltering.linear()
            // Texture alpha needs the blended pass. The slight reduction
            // keeps this out of the opaque pass while preserving full visual
            // intensity from the texture and emissive material.
            Opacity.opacity(0.99)
            muzzle_flash_0_emissive
        }
    }
    let muzzle_flash_1 = T.position(0.0, 0.0, 0.002).scale(0.0, 0.0, 0.0) {
        name = "car_laser_muzzle_flash_1"
        R.square() {
            C.rgba(1.0, 1.0, 1.0, 1.0)
            Texture.with_uri("assets/images/flash_red_1.png")
            TextureFiltering.linear()
            Opacity.opacity(0.99)
            muzzle_flash_1_emissive
        }
    }
    // Squares have a local +Z normal. The vehicle fires along local -Z, so
    // turn the shared flash frame around to present its textured face outward.
    let muzzle_flash = T.position(0.0, 0.0, 0.0).rotation(0.0, 3.14159, 0.0) {
        name = "car_laser_muzzle_flash"
        muzzle_flash_0
        muzzle_flash_1
    }

    // The beam extends along the car's semantic local -Z axis. Nested widths
    // approximate an emissive falloff until a textured beam asset replaces it.
    let laser_beam_glow = T.position(0.0, 0.0, 0.0)
        .rotation(-1.5708, 0.0, 0.0).scale(0.0, 0.0, 0.0) {
        name = "laser_beam_glow"
        T.scale(0.16, laser_length, 1.0) {
            R.square() {
                C.rgba(1.0, 0.06, 0.03, 1.0)
                Opacity.opacity(0.18)
                laser_outer_emissive
            }
        }
        T.position(0.0, 0.0, 0.002).scale(0.08, laser_length, 1.0) {
            R.square() {
                C.rgba(1.0, 0.22, 0.08, 1.0)
                Opacity.opacity(0.38)
                laser_middle_emissive
            }
        }
        T.position(0.0, 0.0, 0.004).scale(0.028, laser_length, 1.0) {
            R.square() {
                C.rgba(1.0, 0.88, 0.58, 1.0)
                Opacity.opacity(0.88)
                laser_core_emissive
            }
        }
    }

    let laser_shot = Animation.paused().length(0.22) {
        Keyframe.at(0.0) {
            muzzle_flash.update_transform(
                [0.0, 0.0, 0.0], [0.0, 3.14159, 0.0], [1.0, 1.0, 1.0]
            )
            muzzle_flash_0.update_transform(
                [0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.90, 0.90, 0.90]
            )
            muzzle_flash_1.update_transform(
                [0.0, 0.0, 0.002], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]
            )
            laser_beam_glow.update_transform(
                [0.0, 0.0, -laser_length * 0.5], [-1.5708, 0.0, 0.0], [1.0, 1.0, 1.0]
            )
            muzzle_flash_0_emissive.set_intensity(8.0)
            muzzle_flash_1_emissive.off()
            laser_outer_emissive.set_intensity(3.0)
            laser_middle_emissive.set_intensity(6.0)
            laser_core_emissive.set_intensity(12.0)
        }
        Keyframe.at(0.05) {
            muzzle_flash_0.update_transform(
                [0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]
            )
            muzzle_flash_1.update_transform(
                [0.0, 0.0, 0.002], [0.0, 0.0, 0.0], [0.90, 0.90, 0.90]
            )
            muzzle_flash_0_emissive.off()
            muzzle_flash_1_emissive.set_intensity(8.0)
        }
        Keyframe.at(0.10) {
            muzzle_flash_0.update_transform(
                [0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]
            )
            muzzle_flash_1.update_transform(
                [0.0, 0.0, 0.002], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]
            )
            laser_beam_glow.update_transform(
                [0.0, 0.0, -laser_length * 0.5], [-1.5708, 0.0, 0.0], [0.0, 0.0, 0.0]
            )
            muzzle_flash_0_emissive.off()
            muzzle_flash_1_emissive.off()
            laser_outer_emissive.off()
            laser_middle_emissive.off()
            laser_core_emissive.off()
        }
    }

    fn fire_laser() {
        if laser_placement.ready { laser_shot.play() }
    }

    let laser_origin = T {
        name = "car_laser_origin"
        muzzle_flash
        laser_beam_glow
    }

    let car_root = display_car(
        "left_display_car",
        [-19.0, -0.75, -1.5],
        0.30,
        "left_display_car_cxr_mount",
        [laser_origin, laser_shot],
    )
    car_root

    on(car_root, "MountStarted", fn(event) {
        vehicle_state.mounted = true
        vehicle_state.left_stick = [0.0, 0.0]
    })

    on(car_root, "MountEnded", fn(event) {
        vehicle_state.mounted = false
        vehicle_state.left_stick = [0.0, 0.0]
        vehicle_state.right_grip_held = false
    })

    on(vehicle_controls, "XrAxisChanged", fn(event) {
        if event.control == "LeftStick" {
            vehicle_state.left_stick = event.value
        }
    })

    on(vehicle_controls, "XrButtonDown", fn(event) {
        if event.control == "RightGrip" {
            vehicle_state.right_grip_held = true
        } else if event.control == "RightTrigger" {
            if vehicle_state.mounted && vehicle_state.right_grip_held {
                fire_laser()
            }
        }
    })

    on(vehicle_controls, "XrButtonUp", fn(event) {
        if event.control == "RightGrip" {
            vehicle_state.right_grip_held = false
        }
    })

    let car_model = car_root.query("#car_model")
    let muzzle_origin = car_root.query("#car_laser_origin")
    on_global("FrameTick", fn(event) {
        if !laser_placement.ready {
            let model_box = car_model.local_bounds()
            if model_box {
                let x = (model_box["min"][0] + model_box["max"][0]) * 0.5
                let y = model_box["min"][1] + (model_box["max"][1] - model_box["min"][1]) * muzzle_height_fraction
                let front_z = model_box["min"][2] - muzzle_clearance
                muzzle_origin.update_transform(
                    [x, y, front_z], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0],
                )
                laser_placement.ready = true
            }
        }
        if vehicle_state.mounted {
            let steering = vehicle_state.left_stick[0]
            let throttle = vehicle_state.left_stick[1]
            let stick_length = Math.sqrt(steering * steering + throttle * throttle)
            if stick_length > car_stick_deadzone {
                vehicle_state.yaw = vehicle_state.yaw - steering * car_turn_speed * event.dt_sec
                let distance = throttle * car_drive_speed * event.dt_sec
                vehicle_state.position = [
                    vehicle_state.position[0] - Math.sin(vehicle_state.yaw) * distance,
                    -0.75,
                    vehicle_state.position[2] - Math.cos(vehicle_state.yaw) * distance,
                ]
                car_root.update_transform(
                    vehicle_state.position,
                    [0.0, vehicle_state.yaw, 0.0],
                    [1.0, 1.0, 1.0],
                )
            }
        }
    })
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

XR.on()
