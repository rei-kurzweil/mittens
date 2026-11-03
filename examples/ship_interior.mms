// Retro spaceship interior blockout. Metres, Y=0 is the walkable deck.
// Run: cargo run --release -- load examples/ship_interior.mms
// Bridge faces -Z. Upstairs: washrooms west, Cherry Grove east.
// Cafeteria: 22 m wide, 8.5 m high. Stairs reach the Y=5.2 upper landing.
// Desktop: WASD + mouse. Set either view flag, or both, in this one file.
// Barracks: grab a machine pistol, then use the XR trigger (desktop: Space).
// View presets: desktop=true/xr=false; XR=false/true; both=true/true.
let desktop_view_enabled = false
let xr_view_enabled = true
let xr_avatar_enabled = true
let bloom_enabled = true
let view_far_distance = 1000.0
RendererSettings { window_visible(desktop_view_enabled) window_size(1280, 800) }

import { ship_input_controller } from "../assets/components/ship_input_controller.mms"
import { ship_control_room } from "../assets/components/rooms/ship_control_room.mms"
import { ship_mall_entrance_hallway } from "../assets/components/rooms/ship_mall_entrance_hallway.mms"
import { ship_mall_cafeteria } from "../assets/components/rooms/ship_mall_cafeteria.mms"
import { ship_washroom_passage } from "../assets/components/rooms/ship_washroom_passage.mms"
import { ship_womens_washroom } from "../assets/components/rooms/ship_womens_washroom.mms"
import { ship_park_streaming_gate } from "../assets/components/rooms/ship_park_streaming_gate.mms"
import { ship_park } from "../assets/components/rooms/ship_park.mms"
import { ship_trailer_park } from "../assets/components/rooms/ship_trailer_park.mms"
import { ship_barracks } from "../assets/components/rooms/ship_barracks.mms"
import { ship_space_sky } from "../assets/components/backgrounds/ship_space_sky.mms"
import { bisket_humanoid_bone_map } from "../assets/components/humanoid_bone_maps/bisket.mms"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { vroid_arm_ik_bend_plane_prototype } from "../assets/components/arm_ik/vroid.mms"

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

// All default surfaces, including UI and separately authored props, inherit anime.
// Explicit metal, sky, emission and unlit declarations retain their own contracts.
T {
    name = "ship_interior"
    Material(MaterialDefinition.anime())
    ship_input_controller()
    ship_space_sky()

    ship_control_room()
    ship_mall_entrance_hallway()
    T.position(-1.5, 0.0, 3.0) { ship_barracks() }
    T.position(0.0, 0.0, 6.0) { ship_mall_cafeteria() }
    ship_washroom_passage()
    T.position(-17.0,5.2,12.0).rotation(0,Math.pi,0) { ship_womens_washroom() }
    T.position(43.0, 5.2, 15.0) { ship_park() }
    // Continue through Cherry Grove into the larger vehicle biodome.
    T.position(125.0, 5.2, 16.0) { ship_trailer_park() }
    ship_park_streaming_gate()
    if xr_view_enabled {
        if xr_avatar_enabled {
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
                            AVC.movement_target("[name='ship_player_root']") {
                                name = "ship_avatar_control"
                                initial_yaw(3.1415927)
                                crouch_sole_offsets([0.0, 0.0530325, 0.0874534], [0.0, 0.0530325, 0.0874534])
                                arm_ik(vroid_arm_ik_bend_plane_prototype())
                                hand_rotation_smoothing(220.0)
                                T {
                                    GLTF.new("assets/models/mitten.glb") {
                                        bisket_humanoid_bone_map()
                                        bisket_anime_shading()
                                        HandTracking.new().source("openxr")
                                    }
                                }
                                T.position(0.0, 0.08, 0.12) { CXR.far(view_far_distance) { name = "ship_mitten_xr_camera" Pointer {} } }
                                XRHand.new(true, "Left", "GripAim").laser() {
                                    T { RestAttachment.new("[name='J_Bip_L_Hand']", "[name='J_Bip_L_Middle3']") { Pointer { name = "ship_left_hand_pointer" } } }
                                }
                                XRHand.new(true, "Right", "GripAim").laser() {
                                    T { RestAttachment.new("[name='J_Bip_R_Hand']", "[name='J_Bip_R_Middle3']") { Pointer { name = "ship_right_hand_pointer" } } }
                                }
                            }
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
