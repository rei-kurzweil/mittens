import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"
import { bisket_secondary_motion } from "../assets/components/secondary_motion/bisket.mms"
import { pose as relaxed_pose_factory } from "../assets/components/poses/bisket/000-relaxed.pose.mms"
import { tripod_light } from "../assets/components/tripod_light.mms"
import { button } from "../assets/components/button.mms"

// Desktop Bisket secondary-motion studio. The light fixtures and collision
// playground are deliberately visible so spring and locomotion behavior can be
// inspected without an XR runtime.

RendererSettings { window_size(1280, 720) }
BGC.rgba(0.006, 0.012, 0.055, 1.0)
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

fn static_cube(cube_name, position, size, color) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        name = cube_name
        Collision.static() {
            CollisionShape.cube([size[0] * 0.5, size[1] * 0.5, size[2] * 0.5])
        }
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) }
    }
}

BG.occlusion_and_lighting() {
    star_kawaii_background([1.0, 0.82, 0.12, 1.0])
}

tripod_light("studio_key_light", [-4.2, 0.0, 2.8], [0.0, 1.25, 0.0], SL.color(1.0, 0.78, 0.62).intensity(6.0).distance(11.0).angle(0.62).penumbra(0.35))
tripod_light("studio_fill_light", [4.0, 0.0, 1.4], [0.0, 1.25, 0.0], SL.color(0.48, 0.68, 1.0).intensity(4.5).distance(11.0).angle(0.62).penumbra(0.35))
tripod_light("studio_rim_light", [1.8, 0.0, -4.2], [0.0, 1.25, 0.0], SL.color(1.0, 0.42, 0.78).intensity(5.0).distance(11.0).angle(0.62).penumbra(0.35))

// The floor's top face is the world-space y=0 ground plane.
static_cube("studio_floor", [0.0, -0.05, 0.0], [18.0, 0.1, 18.0], [0.025, 0.035, 0.075])

// Stable, static piles: useful obstacles without rigid-body stack noise.
static_cube("pile_a_base_left",  [-2.8, 0.40, -1.7], [0.85, 0.80, 0.85], [0.95, 0.25, 0.38])
static_cube("pile_a_base_right", [-1.9, 0.40, -1.7], [0.85, 0.80, 0.85], [1.00, 0.62, 0.15])
static_cube("pile_a_top",        [-2.35, 1.22, -1.7], [0.85, 0.80, 0.85], [1.00, 0.88, 0.22])

static_cube("pile_b_base_left",  [1.7, 0.40, -2.8], [0.9, 0.8, 0.9], [0.18, 0.78, 0.96])
static_cube("pile_b_base_right", [2.65, 0.40, -2.8], [0.9, 0.8, 0.9], [0.28, 0.52, 1.00])
static_cube("pile_b_top",        [2.18, 1.22, -2.8], [0.9, 0.8, 0.9], [0.62, 0.38, 1.00])

static_cube("pile_c_base", [3.2, 0.45, 2.5], [1.25, 0.9, 0.9], [0.20, 0.92, 0.62])
static_cube("pile_c_top",  [3.2, 1.37, 2.5], [0.9, 0.9, 0.9], [0.78, 1.00, 0.34])

let avatar_gltf = GLTF.new("assets/models/bisket.11.0.glb") {
    // Direct pose children establish one-shot startup overlays after import.
    relaxed_pose_factory()
    EM.on()
    // false means "use Bisket defaults": 14 hair chains and two bust chains.
    bisket_secondary_motion(false)
}

let camera_view_state = { first_person = false }
let camera_view_toggle = button("toggle camera view", {
    background_color = [0.16, 0.48, 0.88, 0.92]
    color = [1.0, 1.0, 1.0, 1.0]
})

// The camera and its control travel together when the rig is reparented.
let desktop_camera_rig = T {
    name = "desktop_camera_rig"
    C3D { Pointer {} }
    T.position(-0.52, -0.30, -1.0).scale(0.035, 0.035, 0.035) {
        LayoutRoot {
            available_width(24.0)
            available_height(5.0)
            camera_view_toggle
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

ED.active() {
    I.speed(2.2) {
        name = "desktop_avatar_input"
        InputTransformMode.forward_z() {
            roll_axis_y()
            fps_rotation()
        }
        T.position(0.0, 1.6, 1.0) {
            name = "avatar_head_driver"
            AVC {
                head_bone("J_Bip_C_Head")
                initial_yaw(3.14159)
                T { avatar_gltf }
            }
        }
    }
}

// Keep the workspace outside the editable scene so bounds inspection discovers
// Bisket without allowing the panel itself to become an editor target.
T.position(-2.25, 2.85, 0.0) {
    EditorUI {
        panels([{
            panel = "settings"
            config = {
                show_armature = true
                show_bounds = true
                show_colliders = true
                show_gltf_colliders = true
            }
        }])
    }
}

// Fixed third-person desktop camera slot; movement controls only the avatar driver.
// C3D views along local -Z, so use an explicit downward pitch instead of
// Transform.looking_at(), which aligns local +Z toward its target.
let fixed_camera_slot = T.position(0.0, 2.0, 5.5).rotation(-0.20, 0.0, 0.0) {
    name = "fixed_camera_slot"
    desktop_camera_rig
}
fixed_camera_slot

on(camera_view_toggle, "Click", fn(event) {
    if camera_view_state.first_person {
        fixed_camera_slot.attach(desktop_camera_rig)
        camera_view_state.first_person = false
        print("camera attached to fixed_camera_slot")
    } else {
        first_person_camera_slot.attach(desktop_camera_rig)
        camera_view_state.first_person = true
        print("camera attached to first_person_camera_slot")
    }
})
