import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"
import { voxel_terrain } from "../assets/components/floors/voxel_terrain.mms"

// Desktop control for bisket-vr-demo. It retains the terrain, animation, render graph,
// and two Bisket model instances without initializing any XR systems.
//
// Run with:
//   CAT_PROFILE_SPATIAL=1 cargo run --release --example bisket-desktop-demo

RendererSettings {
    window_size(640, 480)
}

BGC { C.rgba(0.9, 0.5, 0.3, 1.0) }
AL.rgb(0.10, 0.11, 0.16)
Clock.bpm(60) {}

RenderGraph {
    EmissivePass {
        BlurPass {
            radius_ndc(0.06)
            half_res(true)
        }
    }
    Bloom {
        intensity(0.90)
        radius_ndc(0.06)
        emissive_scale(1.2)
        half_res(true)
    }
}

T.position(0.15, -0.45, 1.0) {
    DL {
        intensity(1.1)
        color(1.0, 0.98, 0.95)
    }
}

T.position(0.0, -6.0, 0.0) {
    voxel_terrain()
}

T.position(0.0, 2.15, -7.2).scale(8.8, 3.6, 0.24) {
    R.cube() { C.rgba(0.95, 0.55, 0.22, 1.0) }
}

let repro_cube_a_transform = T.position(-0.9, -0.44, -1.0) {
    name = "repro_cube_a_transform"
    Transition {
        duration_beats(1.0)
        ease_in_out_sine()
        replace_same_target()
    }
    T.scale(0.50, 0.50, 0.50) {
        R.cube() {
            C.rgba(1.0, 0.88, 0.15, 1.0)
            EM.on()
            Raycastable.enabled()
        }
    }
}

let repro_cube_b_transform = T.position(0.0, -0.44, -0.7) {
    name = "repro_cube_b_transform"
    Transition {
        duration_beats(1.0)
        ease_in_out_sine()
        replace_same_target()
    }
    T.scale(0.50, 0.50, 0.50) {
        R.cube() {
            C.rgba(1.0, 0.35, 0.78, 1.0)
            EM.on()
            Raycastable.enabled()
        }
    }
}

let repro_cube_c_transform = T.position(0.9, -0.44, -1.1) {
    name = "repro_cube_c_transform"
    Transition {
        duration_beats(1.0)
        ease_in_out_sine()
        replace_same_target()
    }
    T.scale(0.50, 0.50, 0.50) {
        R.cube() {
            C.rgba(0.10, 0.95, 1.0, 1.0)
            EM.on()
            Raycastable.enabled()
        }
    }
}

let repro_rotating_parent = T.position(2.0, 0.15, 1.0) {
    name = "repro_rotating_parent"
    Transition {
        duration_beats(1.0)
        ease_in_out_sine()
        replace_same_target()
    }
    repro_cube_a_transform
    repro_cube_b_transform
    repro_cube_c_transform
}
repro_rotating_parent

Animation.looping().length(4.0) {
    Keyframe.at(0.0) {
        repro_rotating_parent.update_transform([2.0, 0.15, 1.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
        repro_cube_a_transform.update_transform([-0.9, -0.44, -1.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
        repro_cube_b_transform.update_transform([0.0, -0.44, -0.7], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
        repro_cube_c_transform.update_transform([0.9, -0.44, -1.1], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
    }
    Keyframe.at(1.0) {
        repro_rotating_parent.update_transform([2.0, 0.15, 1.0], [0.0, 0.65, 0.0], [1.0, 1.0, 1.0])
        repro_cube_a_transform.update_transform([-1.25, -0.10, -0.55], [0.0, 1.570795, 0.0], [1.0, 1.0, 1.0])
        repro_cube_b_transform.update_transform([0.0, 0.45, -1.35], [1.570795, 0.0, 0.0], [1.0, 1.0, 1.0])
        repro_cube_c_transform.update_transform([1.25, -0.20, -0.55], [0.0, 0.0, 1.570795], [1.0, 1.0, 1.0])
    }
    Keyframe.at(2.0) {
        repro_rotating_parent.update_transform([2.0, 0.15, 1.0], [0.0, 1.35, 0.0], [1.0, 1.0, 1.0])
        repro_cube_a_transform.update_transform([-0.35, 0.55, -1.45], [0.0, 3.14159, 0.0], [1.0, 1.0, 1.0])
        repro_cube_b_transform.update_transform([0.0, -0.55, -0.25], [3.14159, 0.0, 0.0], [1.0, 1.0, 1.0])
        repro_cube_c_transform.update_transform([0.35, 0.55, -1.45], [0.0, 0.0, 3.14159], [1.0, 1.0, 1.0])
    }
    Keyframe.at(3.0) {
        repro_rotating_parent.update_transform([2.0, 0.15, 1.0], [0.0, 2.10, 0.0], [1.0, 1.0, 1.0])
        repro_cube_a_transform.update_transform([-1.35, -0.25, -1.35], [0.0, 4.712385, 0.0], [1.0, 1.0, 1.0])
        repro_cube_b_transform.update_transform([0.0, 0.30, 0.10], [4.712385, 0.0, 0.0], [1.0, 1.0, 1.0])
        repro_cube_c_transform.update_transform([1.35, -0.25, -1.35], [0.0, 0.0, 4.712385], [1.0, 1.0, 1.0])
    }
}

BG.occlusion_and_lighting() {
    star_kawaii_background([1.0, 0.88, 0.42, 1.0])
    T.position(2.0, 1.5, -8.0).scale(3.5, 3.5, 3.5) {
        R.circle2d() {
            C.rgba(1.0, 0.85, 0.15, 1.0)
            EM.on()
        }
    }
}

// Scene subject: matches the visible avatar polygon/skin workload from the VR demo.
ED.active() {
    T.position(0.0, -0.5, 0.0).rotation(0.0, 3.14159, 0.0) {
        name = "desktop_scene_bisket"
        GLTF.new("assets/models/bisket.11.0.glb") {
            EM.on()
            PoseCapture { label("Bisket Scene") asset_name("bisket_scene") }
        }
    }
}

// Desktop camera rig with a second Bisket instance under it. This intentionally keeps two
// copies of the skinned model loaded/drawn while avoiding AVC and every XR component.
I.speed(2.0) {
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
    }
    T.position(3.0, 1.2, 3.5).rotation(0.0, 0.5, 0.0) {
        name = "desktop_camera_rig"
        Collision.kinematic() {
            CollisionShape.sphere(0.22)
            CollisionResponse.slide() {}
        }
        C3D { Pointer {} }
        T.position(0.0, -1.7, 1.0).rotation(0.0, 3.14159, 0.0) {
            name = "desktop_camera_rig_bisket"
            GLTF.new("assets/models/bisket.11.0.glb") {
                EM.on()
                PoseCapture { label("Bisket Camera Rig") asset_name("bisket_camera_rig") }
            }
        }
    }
}

T.position(0, 2, 0) {
    DL { intensity(0.8) color(1.0, 0.45, 0.85) }
}
T.position(-1, -1, 0) {
    DL { intensity(0.8) color(1.0, 0.9, 1.0) }
}
T.position(1, -1, 0) {
    DL { intensity(0.8) color(1.0, 0.6, 0.15) }
}
