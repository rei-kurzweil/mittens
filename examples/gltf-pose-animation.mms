// Movement-driven sparse GLTF pose animation.
// Required assets (created by the pose-capture workflow):
//   assets/components/poses/bisket/000-relaxed.pose.mms
//   assets/components/poses/bisket/002-running_1.pose.mms
//   assets/components/poses/bisket/003-running_2.pose.mms

// Pose assets export a named factory; they do not emit a positional root component.
import { pose as relaxed_pose_factory } from "../assets/components/poses/bisket/000-relaxed.pose.mms"
import { pose as run_pose_1_factory } from "../assets/components/poses/bisket/002-running_1.pose.mms"
import { pose as run_pose_2_factory } from "../assets/components/poses/bisket/003-running_2.pose.mms"
import { bisket_secondary_motion } from "../assets/components/secondary_motion/bisket.mms"

let relaxed_pose = relaxed_pose_factory()
let run_pose_1 = run_pose_1_factory()
let run_pose_2 = run_pose_2_factory()

RendererSettings { window_size(960, 720) }
BGC.rgba(0.12, 0.14, 0.18, 1.0)
AL.rgb(0.35, 0.35, 0.38)
Clock.bpm(120) {}

T.position(3.0, 5.0, 4.0) { DL {} }

let avatar_gltf = GLTF.new("assets/models/bisket.11.0.glb") {
    EM.on()
    bisket_secondary_motion(false)
}
let avatar_control = AVC {
    head_bone("J_Bip_C_Head")
    initial_yaw(3.14159)
    T { avatar_gltf }
}
let avatar_transform = T.position(0.0, 0.0, 0.0) { avatar_control }

// Input owns the avatar's controlling transform. The camera deliberately lives elsewhere.
I.speed(2.0) {
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
    }
    avatar_transform
}

T.position(0.0, 1.4, 4.5).rotation(-0.12, 0.0, 0.0) {
    name = "fixed_camera_rig"
    C3D { Pointer {} }
}

BG {
    T.position(0.0, -0.55, 0.0).scale(20.0, 0.1, 20.0) {
        R.cube() { C.rgba(0.28, 0.31, 0.35, 1.0) }
    }
}

// Calling loop_anim() restarts from beat zero, so this stays paused until movement begins.
let run_loop = Animation.paused().length(1.0) {
    Keyframe.at(0.0) { run_pose_1.apply(avatar_gltf) }
    Keyframe.at(0.4) { relaxed_pose.apply(avatar_gltf) }
    Keyframe.at(0.5) { run_pose_2.apply(avatar_gltf) }
    Keyframe.at(0.75) { relaxed_pose.apply(avatar_gltf) }
    Keyframe.at(1.0) { run_pose_1.apply(avatar_gltf) }
    Keyframe.at(1.5) { run_pose_2.apply(avatar_gltf) }
    Keyframe.at(1.9) { relaxed_pose.apply(avatar_gltf) }
}
run_loop

// MMS table literals are heap-backed; aliases captured by the handler retain shared state.
let movement = {
    initialized = false
    moving = false
    previous_x = 0.0
    previous_z = 0.0
}

on_global("FrameTick", fn(event) {
    let position = avatar_transform.translation()
    let x = position[0]
    let z = position[2]

    if !movement.initialized {
        movement.initialized = true
        movement.previous_x = x
        movement.previous_z = z
        relaxed_pose.apply(avatar_gltf)
    } else {
        let dx = x - movement.previous_x
        let dz = z - movement.previous_z
        let moving_now = dx * dx + dz * dz > 0.00000001

        if moving_now && !movement.moving {
            run_loop.loop_anim()
        }
        if !moving_now && movement.moving {
            run_loop.pause()
            relaxed_pose.apply(avatar_gltf)
        }

        movement.moving = moving_now
        movement.previous_x = x
        movement.previous_z = z
    }
})
