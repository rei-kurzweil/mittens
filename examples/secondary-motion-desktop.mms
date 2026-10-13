import { vroid_arm_ik_defaults } from "../assets/components/arm_ik/vroid.mms"

let arm_ik = vroid_arm_ik_defaults()

import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { bisket_secondary_motion } from "../assets/components/secondary_motion/bisket.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { pose as relaxed_pose_factory } from "../assets/components/poses/bisket/000-relaxed.pose.mms"
import { stage_with_boxes } from "../assets/components/stage_with_boxes.mms"
import { button } from "../assets/components/button.mms"
import { teleport_pit } from "../assets/components/teleport_pit.mms"
import { camera_icon } from "../assets/components/icons.mms"

// Desktop Bisket secondary-motion studio. The light fixtures and collision
// playground are deliberately visible so spring and locomotion behavior can be
// inspected without an XR runtime.

RendererSettings { window_size(1280, 720) }
BGC.rgba(0.01, 0.01, 0.01, 1.0)
AL.rgb(0.24, 0.27, 0.38)
Clock.bpm(120) {}

RenderGraph {
    EmissivePass {
        BlurPass { radius_ndc(0.045) half_res(true) }
    }
    Bloom {
        intensity(0.8)
        radius_ndc(0.045)
        emissive_scale(1.25)
        half_res(true)
    }
}

BG.occlusion_and_lighting() {
    star_kawaii_background([1.0, 0.82, 0.12, 1.0])
}

stage_with_boxes("secondary_motion_stage")

// Catch players who leave the floor. Destination is the capsule's world center,
// including any movement accumulated on the inner Input driver.
teleport_pit("desktop_teleport_pit", [0.0, -12.0, 0.0], [120.0, 12.0, 120.0], [0.0, 1.2, 1.0], "spikes")

let avatar_gltf = GLTF.new("assets/models/bisket.glb") {
    bisket_anime_shading()
    // Direct pose children establish one-shot startup overlays after import.
    relaxed_pose_factory()
    EM.on()
    bisket_colliders()
    // false means "use Bisket defaults": 14 hair chains and two bust chains.
    bisket_secondary_motion(false)
}

let camera_view_state = { first_person = false }
let camera_view_toggle = button("toggle camera view", {
    compact = false
    background_color = [0.16, 0.48, 0.88, 0.92]
    color = [1.0, 1.0, 1.0, 1.0]
})

let jump_button = button("jump (Space)", {
    compact = false
    background_color = [0.16, 0.68, 0.40, 0.92]
    color = [1.0, 1.0, 1.0, 1.0]
})

// The camera and its control travel together when the rig is reparented.
let desktop_camera_rig = T {
    name = "desktop_camera_rig"
    C3D { Pointer {} }
    T.position(-0.52, -0.30, -1.0).scale(0.035, 0.035, 0.035) {
        LayoutRoot {
            available_width(24.0)
            available_height(10.0)
            camera_view_toggle
            jump_button
        }
    }
}

// Bisket's head-local +Z points toward the face. C3D views along local -Z, so
// the half-turn makes camera forward follow that authored head direction.
let first_person_camera_slot = T.position(0.0, 0.08, 0.06).rotation(0.0, 3.14159, 0.0) {
    name = "first_person_camera_slot"
}

on(avatar_gltf, "GLTFInitialized", fn(event) {

    let head = event.gltf.query("#J_Bip_C_Head")
    if head {
        head.attach(first_person_camera_slot)
    } else {
        print("GLTFInitialized: expected Bisket head bone #J_Bip_C_Head was not found")
    }
})

// Gravity owns falling placement; Input retains desktop movement/look on the
// inner head driver. Capsule correction targets the falling root explicitly.
let avatar_motion = Velocity {
    name = "desktop_avatar_velocity"
    T.position(0.0, 1.0, 0.0) {
        name = "desktop_avatar_grounding_root"
        I.speed(2.2) {
            name = "desktop_avatar_input"
            InputTransformMode.forward_z() {
                roll_axis_y()
                fps_rotation()
            }
            T.position(0.0, 1.6, 1.0) {
                name = "avatar_head_driver"
                AVC {
                    name = "desktop_avatar_control"
                    movement_target("[name='desktop_avatar_grounding_root']")
                    left_two_bone_ik(arm_ik.left)
                    right_two_bone_ik(arm_ik.right)
                    initial_yaw(3.14159)
                    T { avatar_gltf }
                }
            }
        }
    }
}
ED.active() {
    Gravity.enabled(false).coefficient(1.0) { name = "player_gravity" avatar_motion }
}

fn jump() {
    if avatar_motion.grounded() {
        avatar_motion.translate_world([0.0, 4.5, 0.0])
    }
}
on_global("KeyDown", fn(event) {
    if event.code == "Space" { jump() }
})
on(jump_button, "Click", fn(event) { jump() })

// Keep the workspace outside the editable scene so bounds inspection discovers
// Bisket without allowing the panel itself to become an editor target.
T.position(-2.25, 2.85, 0.0) {
    EditorUI {
        panels([{
            panel = "settings"
            config = {
                show_armature = true
                show_bounds = true
                show_cameras = true
                show_zones = true
            }
        }])
    }
}

// Fixed third-person desktop camera slot; movement controls only the avatar driver.
// C3D views along local -Z, so use an explicit downward pitch instead of
// Transform.looking_at(), which aligns local +Z toward its target.
let fixed_camera_icon = T.scale(0.45, 0.45, 0.45) {
    name = "fixed_camera_slot_icon"
    camera_icon()
}
let hidden_camera_icon_parking = T.scale(0.0, 0.0, 0.0) {
    name = "hidden_camera_icon_parking"
    fixed_camera_icon
}
hidden_camera_icon_parking

let fixed_camera_slot = T.position(0.0, 2.0, 5.5).rotation(-0.20, 0.0, 0.0) {
    name = "fixed_camera_slot"
    Draggable {}
    desktop_camera_rig
}
fixed_camera_slot

on(camera_view_toggle, "Click", fn(event) {
    if camera_view_state.first_person {
        fixed_camera_slot.attach(desktop_camera_rig)
        hidden_camera_icon_parking.attach(fixed_camera_icon)
        camera_view_state.first_person = false
        print("camera attached to fixed_camera_slot")
    } else {
        first_person_camera_slot.attach(desktop_camera_rig)
        fixed_camera_slot.attach(fixed_camera_icon)
        camera_view_state.first_person = true
        print("camera attached to first_person_camera_slot")
    }
})

// Falling starts only after AVC's generated slide capsule is usable.
let gravity_avatar = query("[name='desktop_avatar_control']")
on(gravity_avatar, "DataEvent", fn(event) {
    if event == "CapsuleReady" {
        query("[name='player_gravity']").set_enabled(true)
    }
})
// Also support registering this policy after the readiness transition.
if gravity_avatar.capsule_ready() {
    query("[name='player_gravity']").set_enabled(true)
}
