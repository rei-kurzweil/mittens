// mittens-corp-desktop — desktop Bisket camera and pose-authoring scene.
//
// Run with:
//   cargo run --release -- load examples/mittens-corp-desktop.mms

import { tripod_light } from "../assets/components/tripod_light.mms"
import { truss } from "../assets/components/truss.mms"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_humanoid_bone_map } from "../assets/components/humanoid_bone_maps/bisket.mms"
import { ambient_eye_saccades } from "../assets/components/animations/ambient_eye_saccades.mms"
import { suspended_platform } from "../assets/components/platforms/suspended_platform.mms"
import { pose as relaxed_pose_factory } from "../assets/components/poses/bisket/000-relaxed.pose.mms"
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

// Keep the studio stage so this tool retains a stable lighting and mirror
// reference while desktop transforms and gizmos are exercised.
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

// Keep the camera outside AVC's direct children. It is attached to this slot
// after Bisket imports, while the direct child of Input remains its desktop
// movement driver.
let desktop_camera_rig = T {
    name = "bisket_desktop_camera_rig"
    C3D {
        // This camera is mounted at Bisket's head, so its pointer must skip
        // the local face/hair before searching the scene behind it.
        Pointer { Raycast.event_driven().min_distance(0.75) {} }
    }
}
let bisket_first_person_camera_slot = T.position(0.0, 0.08, 0.12).rotation(0.0, 3.14159, 0.0) {
    name = "bisket_first_person_camera_slot"
}

// Desktop mouse/WASD moves the direct Input child. The camera follows Bisket's
// mapped head through the GLTFInitialized attachment above.
ED.active() {
    T {
        name = "bisket_desktop_locomotion_root"
        Rider
            .anchor("[name='bisket_first_person_camera_slot']")
            .movement_root("[name='bisket_desktop_locomotion_root']")
            .input("[name='bisket_desktop_input']") {}
        I.speed(2.0) {
            name = "bisket_desktop_input"
            InputTransformMode.forward_z() {
                fps_rotation()
                roll_axis_y()
            }

            T.position(-5.0, 1.65, 4.2) {
                name = "bisket_desktop_driver"
                let bisket_avatar = GLTF.new("assets/models/bisket.glb") {
                bisket_anime_shading()
                bisket_humanoid_bone_map()
                relaxed_pose_factory()
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

                T { bisket_avatar }
                }
                bisket_avatar_control

                // These are the two targets declared by Bisket's explicit
                // humanoid map, queried only in this imported avatar instance.
                on(bisket_avatar, "GLTFInitialized", fn(event) {
                let head = event.gltf.query("[name='J_Bip_C_Head']")
                if head {
                    head.attach(bisket_first_person_camera_slot)
                    bisket_first_person_camera_slot.attach(desktop_camera_rig)
                } else {
                    print("GLTFInitialized: Bisket mapped head bone was not found; desktop camera was not attached")
                }
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

    // The prefab owns the car mesh, entry volume, and mount/dismount points.
    // Left click enters only while the Rider is in its visualized entry zone.
    display_car(
        "left_display_car",
        [-19.0, -0.75, -1.5],
        0.30,
        "left_display_car_desktop_mount",
        [],
    )
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
