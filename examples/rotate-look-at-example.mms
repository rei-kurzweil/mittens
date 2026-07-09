import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"

RendererSettings {
    window_size(1280, 960)
}

BGC.with_occlusion_and_lighting() {
    C.rgba(0.17, 0.12, 0.09, 1.0)
}
AL.rgb(0.28, 0.22, 0.20)

RenderGraph {
    EmissivePass {
        BlurPass {
            radius_ndc(0.06)
            half_res(true)
        }
    }
    Bloom {
        intensity(0.85)
        radius_ndc(0.06)
        emissive_scale(1.15)
        half_res(true)
    }
}

fn hash01(seed) {
    let x = Math.sin(seed * 12.9898 + 78.233) * 43758.5453
    return x - Math.floor(x)
}

fn walk_delta(index, step) {
    // let seed = index + 1.0
    // let yaw = hash01(seed + 3.0) * Math.tau
    // let pitch = (hash01(seed + 5.0) - 0.5) * 0.85

    let cos_pitch = Math.cos(index * 0.1)
    let sin_pitch = Math.sin(index * 0.1)
    let cos_yaw = Math.cos(index * 0.1)
    let sin_yaw = Math.sin(index * 0.1)

    return [
        step * cos_pitch * cos_yaw,
        step * sin_pitch / 2.0,
        step * cos_pitch * sin_yaw,
    ]
}

fn cone_root(x, y, z, cone_radius, cone_length) {
    return T.position(x, y, z).scale(cone_radius, cone_radius, cone_length) {
        R.cone() {
            C.rgba(0.70, 0.98, 0.24, 1.0)
        }
    }
}

fn cone_looking_back(x, y, z, previous_translation, cone_radius, cone_length) {
    return T
        .position(x, y, z)
        .looking_at(previous_translation)
        .scale(cone_radius, cone_radius, cone_length) {
        R.cone() {
            C.rgba(0.70, 0.98, 0.24, 1.0)
        }
    }
}

let moon = T.position(3.9, 7.8, -10.0).scale(2.0, 2.0, 1.0).rotation(0.0, 0.0, 0.25) {
    R.circle2d(0.5, 64) {
        C.rgba(0.96, 0.77, 0.61, 1.0)
        EM.on() {
            intensity(1.6)
        }
    }
}

BG.occlusion_and_lighting() {
    ED {
        moon
    }

    star_kawaii_background([0.96, 0.72, 0.48, 1.0])
}

T.position(1.8, 2.4, 1.2) {
    DL {
        intensity(1.2)
        color(1.0, 0.90, 0.80)
    }
}

T.position(-1.5, 1.3, -0.9) {
    DL {
        intensity(0.95)
        color(1.0, 1.0, 1.0)
    }
}

I.speed(1.5) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }
    T.position(0.0, 0.2, 8.5) {
        C3D {
            Pointer {}
        }
    }
}

T.position(0.0, -2.3, 0.0).scale(10000, 0.1, 10000) {
    R.cube() {
        C.rgba(0.97, 0.60, 0.76, 1.0)
    }
}

let cube_radius = 0.35
let walk_step = cube_radius * 4.0
let cone_radius = cube_radius * 0.9
let cone_length = cube_radius * 2.5
let previous_translation = [0.0, -0.6, -2.0]

cone_root(
    previous_translation[0],
    previous_translation[1],
    previous_translation[2],
    cone_radius,
    cone_length,
)

for i in range(256) {
    let delta = walk_delta(i, walk_step)
    let next_translation = [
        previous_translation[0] + delta[0],
        previous_translation[1] + delta[1],
        previous_translation[2] + delta[2],
    ]

    cone_looking_back(
        next_translation[0],
        next_translation[1],
        next_translation[2],
        previous_translation,
        cone_radius,
        cone_length,
    )

    previous_translation = next_translation
}
