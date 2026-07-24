// XR grip-ray grabbing demo. Aim either controller ray at a colored object,
// squeeze grip, and the object attaches to the controller and levitates to a
// safe clearance. Move/rotate the controller, then release. Trigger does not grab.
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { pose as relaxed_pose_factory } from "../assets/components/poses/bisket/000-relaxed.pose.mms"
import { tripod_light } from "../assets/components/tripod_light.mms"
import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"

RendererSettings { window_size(960, 640) }
BGC.rgba(0.035, 0.045, 0.085, 1.0)
AL.rgb(0.20, 0.22, 0.30)

RenderGraph {
    EmissivePass { BlurPass { radius_ndc(0.045) half_res(true) } }
    Bloom { intensity(0.8) emissive_scale(1.2) }
}

fn grabbable_cube(cube_name, position, size, color) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        name = cube_name
        Grabbable {}
        Collision.static() {
            CollisionShape.cube([size[0] * 0.5, size[1] * 0.5, size[2] * 0.5])
        }
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) }
    }
}

BG.occlusion_and_lighting() {
    star_kawaii_background([1.0, 0.68, 0.12, 1.0])
}

tripod_light("studio_key_light", [-4.2, 0.0, 2.8], [0.0, 1.25, -1.5], SL.color(1.0, 0.78, 0.62).intensity(6.0).distance(11.0).angle(0.62).penumbra(0.35))
tripod_light("studio_fill_light", [4.0, 0.0, 1.4], [0.0, 1.25, -1.5], SL.color(0.48, 0.68, 1.0).intensity(4.5).distance(11.0).angle(0.62).penumbra(0.35))
tripod_light("studio_rim_light", [1.8, 0.0, -4.2], [0.0, 1.25, -1.5], SL.color(1.0, 0.42, 0.78).intensity(5.0).distance(11.0).angle(0.62).penumbra(0.35))

T.position(-2.75, 2.8, -1.5) {
    EditorUI {
        panels([{
            panel = "settings"
        }])
    }
}

// Desktop fallback camera.
I.speed(2.0) {
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
    }
    T.position(0.0, 1.8, 6.0) {
        C3D { Pointer {} }
    }
}

// Tracked Bisket avatar. The left stick moves the entire avatar/controller rig,
// while the secondary-motion chains respond to both locomotion and body motion.
ED.active() {
    name = "xr_grab_editor_scene"

    T {
        name = "xr_grab_locomotion_root"
        InputXR.on() {
            InputXRGamepad {
                locomotion()
                speed(1.5)
            }
            T {
                name = "xr_grab_avatar_driver"
                AVC {
                    head_bone("J_Bip_C_Head")
                    camera_bone("J_Bip_C_Head")
                    left_hand_bone("J_Bip_L_Hand")
                    right_hand_bone("J_Bip_R_Hand")
                    initial_yaw(3.14159)
                    left_arm_pole_direction([1, -0.35, 1])
                    right_arm_pole_direction([-1, -0.35, 1])
                    hand_rotation_smoothing(220.0)

                    T {
                        GLTF.new("assets/models/bisket.glb") {
                            // Establish a relaxed lower-body/rest posture before XR
                            // head and hand tracking take ownership of tracked joints.
                            relaxed_pose_factory()
                            EM.on()
                            bisket_colliders()
                            bisket_shirt_physics(false)
                        }
                    }

                    T.position(0.0, 0.08, 0.12) {
                        name = "xr_grab_camera"
                        CXR { Pointer {} }
                    }

                    XRHand.new(true, Left, Grip)
                        .laser_from_avatar_finger("[name='J_Bip_L_Middle1']", "[name='J_Bip_L_Middle2']", "[name='J_Bip_L_Middle3']") {
                        T { Pointer {} }
                    }
                    XRHand.new(true, Right, Grip)
                        .laser_from_avatar_finger("[name='J_Bip_R_Middle1']", "[name='J_Bip_R_Middle2']", "[name='J_Bip_R_Middle3']") {
                        T { Pointer {} }
                    }
                }
            }
        }
    }
    XR.on()

    T.position(0.0, -0.10, 0.0).scale(10.0, 0.20, 10.0) {
        R.cube() { C.rgba(0.10, 0.14, 0.22, 1.0) }
    }

    T.position(-1.4, 1.1, -1.8) {
        name = "grab_red_outer"
        Grabbable {}
        T.rotation(0.0, 0.3, 0.0) {
            T.scale(0.75, 0.75, 0.75) {
                R.cube() { C.rgba(0.95, 0.22, 0.25, 1.0) EM.on() }
            }
        }
    }

    T.position(0.0, 1.2, -2.2) {
        name = "grab_nested_outer"
        Grabbable {}
        T.position(0.0, 0.25, 0.0) {
            T.scale(0.55, 0.55, 0.55) {
                R.sphere() { C.rgba(0.20, 0.78, 1.0, 1.0) EM.on() }
            }
            T.position(0.0, -0.65, 0.0).scale(0.22, 0.75, 0.22) {
                R.cube() { C.rgba(0.15, 0.50, 0.95, 1.0) }
            }
        }
    }

    T.position(1.5, 1.0, -1.7) {
        name = "grab_gold_outer"
        Grabbable {}
        T.scale(0.70, 0.70, 0.70) {
            R.cone() { C.rgba(1.0, 0.72, 0.12, 1.0) EM.on() }
        }
    }

    // Colorful collision piles from the desktop secondary-motion playground.
    grabbable_cube("pile_a_base_left",  [-2.8, 0.40, -1.7], [0.85, 0.80, 0.85], [0.95, 0.25, 0.38])
    grabbable_cube("pile_a_base_right", [-1.9, 0.40, -1.7], [0.85, 0.80, 0.85], [1.00, 0.62, 0.15])
    grabbable_cube("pile_a_top",        [-2.35, 1.22, -1.7], [0.85, 0.80, 0.85], [1.00, 0.88, 0.22])

    grabbable_cube("pile_b_base_left",  [1.7, 0.40, -2.8], [0.9, 0.8, 0.9], [0.18, 0.78, 0.96])
    grabbable_cube("pile_b_base_right", [2.65, 0.40, -2.8], [0.9, 0.8, 0.9], [0.28, 0.52, 1.00])
    grabbable_cube("pile_b_top",        [2.18, 1.22, -2.8], [0.9, 0.8, 0.9], [0.62, 0.38, 1.00])

    grabbable_cube("pile_c_base", [3.2, 0.45, 2.5], [1.25, 0.9, 0.9], [0.20, 0.92, 0.62])
    grabbable_cube("pile_c_top",  [3.2, 1.37, 2.5], [0.9, 0.9, 0.9], [0.78, 1.00, 0.34])

    T.position(-2.8, 2.7, -2.5).scale(0.018, 0.018, 0.018) {
        Text { "Left stick: move • GRIP: attach/release • trigger: drag Draggable only" }
    }
}
