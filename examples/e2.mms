import { vroid_arm_ik_defaults } from "../assets/components/arm_ik/vroid.mms"

let arm_ik = vroid_arm_ik_defaults()

// e2 — desktop still life: first-person Bisket, a mirror, estradiol tablet,
// an intentionally simple cube room, and a reusable implicit-cloud sky.
//
// Run with:
//   cargo run --release -- load examples/e2.mms

import { clouds } from "../assets/components/backgrounds/clouds.mms"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { bisket_secondary_motion } from "../assets/components/secondary_motion/bisket.mms"
import { pose as relaxed_pose_factory } from "../assets/components/poses/bisket/000-relaxed.pose.mms"
import { tripod_light } from "../assets/components/tripod_light.mms"



RendererSettings { window_size(1280, 720) }
BGC.rgba(0.055, 0.070, 0.105, 1.0)
AL.rgb(0.20, 0.23, 0.30)

RenderGraph {
    EmissivePass { BlurPass { radius_ndc(0.045) half_res(true) } }
    Bloom { intensity(0.70) radius_ndc(0.045) emissive_scale(1.20) half_res(true) }
}

// The background is a sky layer; its factory puts each cluster well outside
// the room. One periwinkle color keeps this first reusable version coherent.
BG.occlusion_and_lighting() {
    clouds([0.70, 0.80, 1.0, 1.0], 11.0, 9.0, 0.78)
    T.position(-0.45, 1.0, 0.30) { DL.color(0.88, 0.92, 1.0).intensity(1.25) }
    T.position(0.70, 0.35, -0.50) { DL.color(1.0, 0.68, 0.78).intensity(0.75) }
}

fn room_cube(cube_name, position, size, color) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        name = cube_name
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) }
    }
}

// A horizontal LayoutRoot is a floor experiment: layout's authored XY plane is
// turned onto world XZ.
// T.position(-6.0, 0.01, 6.0).rotation(-1.5708, 0.0, 0.0).scale(0.10, 0.10, 0.10) {
//     LayoutRoot {
//         name = "e2_floor_layout"
//         available_width(120.0)
//         available_height(120.0)
//         T {
//             name = "e2_floor_styled_surface"
//             Style {
//                 display("block")
//                 width(120.0)
//                 height(120.0)
//                 background_color([0.11, 0.13, 0.18, 1.0])
//                 background_z(-0.01)
//             }
//         }
//     }
// }
T.position(0,0,0).scale(24, 24, 0.1).rotation(-1.5708, 0, 0) {
    name = "e2_floor"
    R.cube() { C.rgba(0.11, 0.13, 0.18, 1.0) }
}

// Cubes make the first room deliberately legible. The front is open as an
// entrance. The back wall is split into a window frame behind the mirror so
// the mirror capture can be tested against geometry close behind its plane.
room_cube("e2_back_wall_left",  [-4.10, 2.0, -5.9], [3.80, 4.0, 0.20], [0.18, 0.20, 0.28])
room_cube("e2_back_wall_right", [ 4.10, 2.0, -5.9], [3.80, 4.0, 0.20], [0.18, 0.20, 0.28])
room_cube("e2_back_window_lower", [0.0, 0.65, -5.9], [4.40, 1.30, 0.20], [0.18, 0.20, 0.28])
room_cube("e2_back_window_upper", [0.0, 3.30, -5.9], [4.40, 1.40, 0.20], [0.18, 0.20, 0.28])
room_cube("e2_left_wall",  [-5.9, 2.0, 0.0], [0.20, 4.0, 12.0], [0.16, 0.18, 0.26])
room_cube("e2_right_wall", [5.9, 2.0, 0.0], [0.20, 4.0, 12.0], [0.16, 0.18, 0.26])
room_cube("e2_entry_left",  [-3.8, 1.15, 5.8], [2.2, 2.3, 0.20], [0.18, 0.20, 0.28])
room_cube("e2_entry_right", [ 3.8, 1.15, 5.8], [2.2, 2.3, 0.20], [0.18, 0.20, 0.28])
room_cube("e2_entry_lintel", [0.0, 2.85, 5.8], [5.4, 0.35, 0.20], [0.18, 0.20, 0.28])
T.position(-0.80, 1.25, 5.67).rotation(0.0, 0.38, 0.0).scale(1.25, 2.50, 0.10) {
    name = "e2_door_ajar"
    R.cube() { C.rgba(0.34, 0.22, 0.34, 1.0) }
}

// A low plinth makes the tablet read as the still-life subject. This follows
// the same direct GLTF.new loading style as the color-cat examples.
room_cube("e2_tablet_plinth", [0.0, 0.42, -1.15], [1.55, 0.84, 1.20], [0.32, 0.23, 0.38])
T.position(0.0, 0.93, -1.15).rotation(0.0, 0.45, 0.0).scale(1.0, 1.0, 1.0) {
    name = "estradiol_tablet"
    Grabbable {}

    

    GLTF.new("assets/models/estradiol-tablet.glb") {
        bisket_anime_shading()
    }
}

T.position(-2, 5, -2) {
    name = "preroll"
    Grabbable {}
    GLTF.new("assets/models/preroll.glb") {
        bisket_anime_shading()
    }
}

T.position(2, 5, -2) {
    name = "broom"
    Grabbable {}
    GLTF.new("assets/models/broom.glb") {
        bisket_anime_shading()
    }
}

// Full-body mirror facing the desktop avatar and the tablet presentation.
T.position(0.0, 1.95, -4.72).scale(2.25, 1.85, 0.08) {
    name = "e2_mirror"
    Grabbable {}
    R.cube() { Mirror.quality(2048) {} }
}

// Two movable studio fixtures aim bright white spotlights at the avatar. Keeping
// the direct lights neutral makes the AnimeShading lit/shade ramp easy to read.
let avatar_light_target = [0.0, 1.5, 2.6]
tripod_light(
    "e2_left_spotlight",
    [-3.2, 0.0, 0.8],
    avatar_light_target,
    SL.color(1.0, 1.0, 1.0).intensity(8.0).distance(12.0).angle(0.62).penumbra(0.25),
)
tripod_light(
    "e2_right_spotlight",
    [3.2, 0.0, 0.8],
    avatar_light_target,
    SL.color(1.0, 1.0, 1.0).intensity(8.0).distance(12.0).angle(0.62).penumbra(0.25),
)

let avatar_gltf = GLTF.new("assets/models/bisket.glb") {
    bisket_anime_shading()
    relaxed_pose_factory()
    EM.on()
    // `false` chooses Bisket's full tuned defaults: hair, bust, and tail.
    // Spring colliders are intentionally omitted for this performance probe.
    bisket_secondary_motion(false)

}

// This rig has only the normal desktop camera and pointer. In particular, this
// example deliberately does not add XR head, eye, or hand tracking.
let desktop_camera_rig = T {
    name = "e2_first_person_camera"
    C3D { Pointer {} }
}
let first_person_camera_slot = T.position(0.0, 0.08, 0.06).rotation(0.0, 3.14159, 0.0) {
    name = "e2_first_person_camera_slot"
}

// Keep this scene's editable workspace deliberately lean: painting, color
// selection, and editor settings only. Omitting world/assets/inspector avoids
// their scene-list work while leaving the visual authoring tools available.
T.position(-3.7, 3.0, -0.5) {
    EditorUI {
        panels([
            { panel = "paint" },
            { panel = "color" },
            { panel = "settings" },
        ])
    }
}

on(avatar_gltf, "GLTFInitialized", fn(event) {
    let head = event.gltf.query("#J_Bip_C_Head")
    if head {
        head.attach(first_person_camera_slot)
        first_person_camera_slot.attach(desktop_camera_rig)
    } else {
        print("e2: expected Bisket head bone #J_Bip_C_Head was not found")
    }
})

// This is the standard desktop avatar-control topology from
// secondary-motion-desktop: WASD locomotion and mouse FPS rotation drive AVC.
ED.active() {
    I.speed(2.2) {
        name = "e2_desktop_avatar_input"
        InputTransformMode.forward_z() {
            roll_axis_y()
            fps_rotation()
        }
        T.position(0.0, 1.6, 2.6) {
            name = "e2_avatar_head_driver"
            AVC {
                left_two_bone_ik(arm_ik.left)
                right_two_bone_ik(arm_ik.right)
                initial_yaw(3.14159)
                T { avatar_gltf }
            }
        }
    }
}
