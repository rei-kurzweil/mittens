// First-person Mitten on the Android renderer: compute-skinned avatar driven
// by the headset and controllers (arm IK, walking IK), no mirror or post effects.
// Launch from the launcher after android/packaging/sync-project.sh.
import { anime_surface } from "../../assets/components/materials/anime_surface.mms"
import { ui_unlit_material } from "../../assets/components/materials/ui_unlit.mms"
import { vroid_arm_ik_bend_plane_prototype } from "../../assets/components/arm_ik/vroid.mms"
import { ambient_eye_saccades } from "../../assets/components/animations/ambient_eye_saccades.mms"
import { mitten_model } from "../../assets/components/characters/mitten.mms"

BGC.rgba(1.0, 0.80, 0.86, 1.0)
AL.rgb(0.38, 0.31, 0.39)
// The avatar's anime shading uses light facing only; keep one key light.
T.position(23, 24, 50) { DL.color(1.0, 0.88, 0.80).intensity(1.4) }

let floor_material = anime_surface()
let pointer_material = ui_unlit_material()

// The floor's top face is the world-space y=0 ground plane.
T.position(0.0, -0.05, 0.0).scale(14.0, 0.1, 14.0) {
    name = "mitten_fp_floor"
    R.cube() { floor_material C.rgba(0.97, 0.91, 0.87, 1.0) }
    Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.8) {} }
}
// A few reference blocks for judging scale and hand placement.
for block in [[-1.2, 0.45, -1.6, 0.53, 0.78, 0.92], [1.3, 0.35, -2.2, 0.93, 0.64, 0.74], [0.0, 0.5, -3.0, 0.98, 0.86, 0.55]] {
    T.position(block[0], block[1], block[2]).scale(0.9, block[1] * 2.0, 0.9) {
        R.cube() { floor_material C.rgba(block[3], block[4], block[5], 1.0) }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.8) {} }
    }
}

fn controller_pointer(label) {
    return T {
        RestAttachment.new("[name='J_Bip_" + label + "_Hand']", "[name='J_Bip_" + label + "_Middle3']") {
            Pointer {}
        }
    }
}

// The Android host supplies eye-level LOCAL head and controller poses. AVC
// hangs the body below the head; its slide capsule settles the locomotion
// root onto the floor once gravity is enabled.
let player_motion = Velocity {
    name = "mitten_fp_root_velocity"
    T.position(0.0, 1.7, 1.0) {
        name = "mitten_fp_root"
        InputXR.on() {
            InputXRGamepad { hand("left") locomotion() speed(1.5) }
            T {
                name = "mitten_fp_driver"
                // No OpenXR hand tracking on the Focus 3 host; controllers drive the arms.
                let avatar = mitten_model("mitten_fp_avatar", { xr = false }).model
                let control = AVC.movement_target("[name='mitten_fp_root']") {
                    name = "mitten_fp_avatar_control"
                    crouch_enabled(true)
                    crouch_sole_offsets([0.0, 0.0530325, 0.0874534], [0.0, 0.0530325, 0.0874534])
                    WalkingIK { step_scale(1.3) }
                    initial_yaw(3.14159)
                    arm_ik(vroid_arm_ik_bend_plane_prototype())
                    hand_rotation_smoothing(220.0)
                    T { avatar }
                    T.position(0.0, 0.08, 0.12) {
                        name = "mitten_fp_head_camera_anchor"
                        CXR {}
                    }
                    XRHand.new(true, "Left", "GripAim") { controller_pointer("L") }
                    XRHand.new(true, "Right", "GripAim") { controller_pointer("R") }
                }
                control
                on(avatar, "GLTFInitialized", fn(event) {
                    let left_eye = event.gltf.query("[name='J_Adj_L_FaceEye']")
                    let right_eye = event.gltf.query("[name='J_Adj_R_FaceEye']")
                    if left_eye && right_eye {
                        control.attach(ambient_eye_saccades(left_eye, right_eye, 2.0))
                    }
                })
            }
        }
    }
}
Gravity.enabled(false) { name = "mitten_fp_gravity" player_motion }
on_global("XrButtonDown", fn(event) {
    if event.control == "ButtonB" && player_motion.grounded() {
        player_motion.translate_world([0.0, 4.5, 0.0])
    }
})

// Enable falling only when AVC's generated slide capsule is ready.
let avatar_control = query("[name='mitten_fp_avatar_control']")
on(avatar_control, "DataEvent", fn(event) {
    if event == "CapsuleReady" { query("[name='mitten_fp_gravity']").set_enabled(true) }
})
if avatar_control.capsule_ready() { query("[name='mitten_fp_gravity']").set_enabled(true) }
