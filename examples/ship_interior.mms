// Retro spaceship interior blockout. Metres, Y=0 is the walkable deck.
// Run: cargo run --release -- load examples/ship_interior.mms
// Bridge faces -Z. Upstairs: washrooms west, Cherry Grove east.
// Cafeteria: 22 m wide, 8.5 m high. Stairs reach the Y=5.2 upper landing.
// Desktop: WASD + mouse. Set either view flag, or both, in this one file.
// Grocery/security shop: grab a machine pistol, then use the XR trigger (desktop: Space).
// View presets: desktop=true/xr=false; XR=false/true; both=true/true.
let desktop_view_enabled = false
let xr_view_enabled = true
let xr_avatar_enabled = true
let bloom_enabled = true
let view_far_distance = 1000.0
// Sakura gradient stops, plus independently tintable baked cloud detail.
let sky_low_color = [1.0, 0.72, 0.66]
let sky_high_color = [0.23, 0.32, 0.62]
let sky_low_point = -0.12
let sky_high_point = 0.82
let sky_cloud_color = [1.0, 0.95, 0.90]
let sky_cloud_strength = 0.65
RendererSettings { window_visible(desktop_view_enabled) window_size(1280, 800) }

import { ship_input_controller } from "../assets/components/ship_input_controller.mms"
import { ship_control_room } from "../assets/components/rooms/ship_control_room.mms"
import { ship_mall_entrance_hallway } from "../assets/components/rooms/ship_mall_entrance_hallway.mms"
import { ship_mall_cafeteria } from "../assets/components/rooms/ship_mall_cafeteria.mms"
import { ship_washroom_passage } from "../assets/components/rooms/ship_washroom_passage.mms"
import { ship_washroom_mirror_zones } from "../assets/components/rooms/ship_washroom_mirror_zones.mms"
import { ship_womens_washroom } from "../assets/components/rooms/ship_womens_washroom.mms"
import { ship_park } from "../assets/components/rooms/ship_park.mms"
import { ship_barracks } from "../assets/components/rooms/ship_barracks.mms"
import { ship_space_sky } from "../assets/components/backgrounds/ship_space_sky.mms"
import { bisket_humanoid_bone_map } from "../assets/components/humanoid_bone_maps/bisket.mms"
import { ship_avatar_material, ship_environment_material } from "../assets/components/materials/ship_anime_materials.mms"
import { vroid_arm_ik_bend_plane_prototype } from "../assets/components/arm_ik/vroid.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { mitten_bow_secondary_motion } from "../assets/components/secondary_motion/mitten-bow.mms"
import { ambient_eye_saccades } from "../assets/components/animations/ambient_eye_saccades.mms"

BGC.rgba(0.005, 0.009, 0.018, 1.0)
// Low global fill preserves the dark hall and upper gallery. Local lights
// below illuminate the bridge, food counters, seating and park independently.
AL.rgb(0.035, 0.045, 0.055)
if bloom_enabled {
    RenderGraph {
        EmissivePass { BlurPass { radius_ndc(0.025) half_res(true) } }
        Bloom { intensity(0.50) radius_ndc(0.025) emissive_scale(1.0) half_res(true) }
    }
} else { RenderGraph.off() }

// Architecture inherits the station environment preset; humanoids select their own.
// Explicit metal, sky, emission and unlit declarations retain their own contracts.
T {
    name = "ship_interior"
    ship_environment_material()
    ship_input_controller()
    ship_space_sky(sky_low_color, sky_high_color, sky_low_point, sky_high_point, sky_cloud_color, sky_cloud_strength)

    ship_control_room()
    ship_mall_entrance_hallway()
    T.position(-1.5, 0.0, 3.0) { ship_barracks() }
    T.position(0.0, 0.0, 6.0) { ship_mall_cafeteria() }
    ship_washroom_passage()
    T.position(-17.0,5.2,12.0).rotation(0,Math.pi,0) { ship_womens_washroom() }
    ship_washroom_mirror_zones()
    T.position(43.0, 5.2, 15.0) { ship_park() }
    if xr_view_enabled {
        if xr_avatar_enabled {
            let microphone = AudioInput { name = "ship_mitten_microphone" }
            let voice_level = Amplitude.rolling_window(0.080).highpass(120.0).highpass_resonance(0.707).from(microphone) {
                name = "ship_mitten_voice_level"
            }
            let player_motion = Velocity {
                name = "ship_player_velocity"
                T.position(0.0, 1.0, -3.0) {
                    name = "ship_player_root"
                    Rider.anchor("[name='ship_mitten_xr_camera']").movement_root("[name='ship_player_root']")
                        .input("[name='ship_xr_locomotion']") {}
                    InputXR.on() {
                        InputXRGamepad { name = "ship_xr_locomotion" locomotion() speed(1.5) }
                        // InputXR drives this direct child as the tracked head frame.
                        T {
                            name = "ship_mitten_xr_driver"
                            let xr_avatar_control = AVC.movement_target("[name='ship_player_root']") {
                                name = "ship_avatar_control"
                                initial_yaw(3.1415927)
                                crouch_sole_offsets([0.0, 0.0530325, 0.0874534], [0.0, 0.0530325, 0.0874534])
                                arm_ik(vroid_arm_ik_bend_plane_prototype())
                                hand_rotation_smoothing(220.0)
                                mouth_open_from_amplitude(voice_level)
                                mouth_open_rms_center_range(0.0475, 0.085)
                                mouth_open_amount(1.0)
                                mouth_open_smoothing(16.0)
                                voice_level
                                T {
                                    GLTF.new("assets/models/mitten.glb") {
                                        name = "ship_xr_mitten"
                                        bisket_humanoid_bone_map()
                                        ship_avatar_material()
                                        HandTracking.new().source("openxr")
                                        MorphTargetMap.new()
                                            .slot("left_eye_blink", "Fcl_EYE_Close_L")
                                            .slot("right_eye_blink", "Fcl_EYE_Close_R")
                                            .slot("viseme_aa", "Fcl_MTH_A")
                                        bisket_colliders()
                                        bisket_shirt_physics(false)
                                        mitten_bow_secondary_motion()
                                    }
                                }
                                // HTC supplies closure only; the idle gaze loop owns direction.
                                XREyeTracking.on().priority(["htc"]).enable_pupil_direction_tracking(false)
                                T.position(0.0, 0.08, 0.12) { CXR.far(view_far_distance) { name = "ship_mitten_xr_camera" Pointer {} } }
                                XRHand.new(true, "Left", "GripAim").laser() {
                                    T { RestAttachment.new("[name='J_Bip_L_Hand']", "[name='J_Bip_L_Middle3']") { Pointer { name = "ship_left_hand_pointer" } } }
                                }
                                XRHand.new(true, "Right", "GripAim").laser() {
                                    T { RestAttachment.new("[name='J_Bip_R_Hand']", "[name='J_Bip_R_Middle3']") { Pointer { name = "ship_right_hand_pointer" } } }
                                }
                            }
                            xr_avatar_control
                            on(query("[name='ship_xr_mitten']"), "GLTFInitialized", fn(event) {
                                let left_eye = event.gltf.query("[name='J_Adj_L_FaceEye']")
                                let right_eye = event.gltf.query("[name='J_Adj_R_FaceEye']")
                                if left_eye && right_eye {
                                    // The default 120 BPM clock runs at two beats per second.
                                    xr_avatar_control.attach(ambient_eye_saccades(left_eye, right_eye, 2.0))
                                } else {
                                    print("Ship Mitten: eye joints were not found for idle gaze")
                                }
                            })
                        }
                    }
                }
            }
            // Wait for the generated locomotion capsule before allowing the avatar to fall.
            Gravity.enabled(false) { name = "ship_player_gravity" player_motion }
            let avatar = query("[name='ship_avatar_control']")
            on(avatar, "DataEvent", fn(event) {
                if event == "CapsuleReady" { query("[name='ship_player_gravity']").set_enabled(true) }
            })
            if avatar.capsule_ready() { query("[name='ship_player_gravity']").set_enabled(true) }
        } else {
            // The tracked head already supplies eye height above this floor origin.
            T.position(0.0, 0.0, -3.0) {
                name = "ship_xr_free_rig"
                T.position(0,0.9,0) { Zone.capsule_y(0.25,0.55) {
                    Collidable.slide().movement_target("/#ship_xr_free_rig") {}
                } }
                Rider.anchor("[name='ship_xr_free_camera']").movement_root("[name='ship_xr_free_rig']")
                    .input("[name='ship_xr_locomotion']") {}
                InputXR.on() {
                    InputXRGamepad { name = "ship_xr_locomotion" locomotion() speed(1.5) }
                    T { CXR.far(view_far_distance) { name = "ship_xr_free_camera" Pointer {} } }
                    XRHand.new(true, "Left", "GripAim").laser() { T { Pointer { name = "ship_left_hand_pointer" } } }
                    XRHand.new(true, "Right", "GripAim").laser() { T { Pointer { name = "ship_right_hand_pointer" } } }
                }
            }
        }
        XR.on()
    } else {
        XR.off()
    }

    if desktop_view_enabled {
        I.speed(2.5) {
            name = "ship_desktop_input"
            InputTransformMode.forward_z() { roll_axis_y() }
            T.position(0.0, 1.65, -2.5) { name = "ship_desktop_camera_rig"
                T.position(0,-0.75,0) { Zone.capsule_y(0.25,0.55) {
                    Collidable.slide().movement_target("/#ship_desktop_camera_rig") {}
                } }
                Rider.anchor("[name='ship_desktop_camera']").movement_root("[name='ship_desktop_camera_rig']")
                    .input("[name='ship_desktop_input']") {}
                C3D.far(view_far_distance) { name = "ship_desktop_camera" Pointer {} } }
        }
    }
}
