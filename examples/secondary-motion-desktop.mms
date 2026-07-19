import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"
import { bisket_secondary_motion } from "../assets/components/secondary_motion/bisket.mms"
import { pose as relaxed_pose_factory } from "../assets/components/poses/bisket/000-relaxed.pose.mms"
import { tripod_light } from "../assets/components/tripod_light.mms"

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

tripod_light("studio_key_light", [-4.2, 0.0, 2.8], [0.0, -0.35, 0.0],
    SL.color(1.0, 0.78, 0.62).intensity(6.0).distance(11.0).angle(0.62).penumbra(0.35))
tripod_light("studio_fill_light", [4.0, 0.0, 1.4], [0.0, -0.35, 0.0],
    SL.color(0.48, 0.68, 1.0).intensity(4.5).distance(11.0).angle(0.62).penumbra(0.35))
tripod_light("studio_rim_light", [1.8, 0.0, -4.2], [0.0, -0.35, 0.0],
    SL.color(1.0, 0.42, 0.78).intensity(5.0).distance(11.0).angle(0.62).penumbra(0.35))

// Floor top is y=-1.6, exactly touching the avatar collider's bottom face.
static_cube("studio_floor", [0.0, -1.65, 0.0], [18.0, 0.1, 18.0], [0.025, 0.035, 0.075])

// Stable, static piles: useful obstacles without rigid-body stack noise.
static_cube("pile_a_base_left",  [-2.8, -1.20, -1.7], [0.85, 0.80, 0.85], [0.95, 0.25, 0.38])
static_cube("pile_a_base_right", [-1.9, -1.20, -1.7], [0.85, 0.80, 0.85], [1.00, 0.62, 0.15])
static_cube("pile_a_top",        [-2.35, -0.38, -1.7], [0.85, 0.80, 0.85], [1.00, 0.88, 0.22])

static_cube("pile_b_base_left",  [1.7, -1.20, -2.8], [0.9, 0.8, 0.9], [0.18, 0.78, 0.96])
static_cube("pile_b_base_right", [2.65, -1.20, -2.8], [0.9, 0.8, 0.9], [0.28, 0.52, 1.00])
static_cube("pile_b_top",        [2.18, -0.38, -2.8], [0.9, 0.8, 0.9], [0.62, 0.38, 1.00])

static_cube("pile_c_base", [3.2, -1.15, 2.5], [1.25, 0.9, 0.9], [0.20, 0.92, 0.62])
static_cube("pile_c_top",  [3.2, -0.23, 2.5], [0.9, 0.9, 0.9], [0.78, 1.00, 0.34])

let avatar_gltf = GLTF.new("assets/models/bisket.11.0.glb") {
    // Direct pose children establish one-shot startup overlays after import.
    relaxed_pose_factory()
    EM.on()
    // false means "use Bisket defaults": 14 hair chains and two bust chains.
    bisket_secondary_motion(false)
}

let avatar_driver = T.position(0.0, -0.8, 1.0) {
    name = "avatar_driver"
    // This input-driven body box starts flush with the floor (bottom y=-1.6).
    // The AVC is offset upward so its own driver remains at head level.
    Collision.kinematic() {
        name = "avatar_body_collider"
        CollisionShape.cube([0.34, 0.8, 0.28])
        KineticResponse.slide() {}
    }
    // Mouse look lives on its own head-level transform. Keeping pitch off the
    // body/collider root prevents the 0.8-unit head offset from orbiting.
    I.speed(0.0) {
        name = "desktop_head_input"
        InputTransformMode.forward_z() { roll_axis_y() fps_rotation() }
        T.position(0.0, 0.8, 0.0) {
            name = "avatar_head_driver"
            AVC {
                head_bone("J_Bip_C_Head")
                initial_yaw(3.14159)
                T { avatar_gltf }
            }
        }
    }
}

I.speed(2.2) {
    name = "desktop_avatar_input"
    // WASD translates the collider/body root using head yaw as its basis, but
    // right-mouse rotation is handled only by desktop_head_input above.
    InputTransformMode.forward_z() {
        rotation_disabled()
        translation_basis("../#avatar_head_driver")
    }
    avatar_driver
}

// Fixed third-person desktop camera; movement controls only the avatar driver.
// C3D views along local -Z, so use an explicit downward pitch instead of
// Transform.looking_at(), which aligns local +Z toward its target.
T.position(0.0, 0.4, 5.5).rotation(-0.20, 0.0, 0.0) {
    name = "desktop_third_person_camera"
    C3D { Pointer {} }
}
