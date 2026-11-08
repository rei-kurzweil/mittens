import { studio_environment_material } from "../assets/components/materials/studio_anime.mms"
// car — first-person desktop vehicle on a studio stage.
//
// Run with:
//   cargo run --release -- load examples/car.mms
//
// W/S drive forward and backward, A/D strafe, and Q/E steer. The Input
// component owns one transform containing both the car and its cockpit camera,
// so the view always follows the vehicle. Enter/exit behavior is intentionally
// deferred; this example starts in the driver's seat.

import { tripod_light } from "../assets/components/tripod_light.mms"
import { truss } from "../assets/components/truss.mms"
import { display_car_anime_material } from "../assets/components/materials/display_car_anime.mms"

RendererSettings { window_size(1440, 810) }
BGC.rgba(0.055, 0.055, 0.060, 1.0)
AL.rgb(0.13, 0.13, 0.15)

RenderGraph {
    EmissivePass { BlurPass { radius_ndc(0.025) half_res(true) } }
    Bloom { intensity(0.42) radius_ndc(0.025) emissive_scale(1.0) half_res(true) }
}

fn stage_box(box_name, position, size, color) {
    return T.position(position[0], position[1], position[2])
        .scale(size[0], size[1], size[2]) {
        name = box_name
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) studio_environment_material() }
    }
}

// A dark floor continues well beyond the stage, especially along X, so the
// edge and descending apron remain readable from the moving car.
stage_box(
    "studio_floor",
    [0.0, -0.92, 1.0],
    [54.0, 0.14, 32.0],
    [0.035, 0.037, 0.043],
)

// The main stage is a long strip across X. At its open front edge (+Z), two
// full-width steps descend to the darker studio floor below.
stage_box("stage_deck",       [0.0,  0.00, -1.5], [32.0, 0.24, 14.0], [0.18, 0.18, 0.20])
stage_box("stage_upper_step", [0.0, -0.24,  5.7], [32.0, 0.28,  0.8], [0.14, 0.14, 0.16])
stage_box("stage_lower_step", [0.0, -0.56,  6.3], [32.0, 0.36,  0.8], [0.10, 0.10, 0.12])

// Backdrop wall, mirror, and ceiling-height truss.
stage_box("stage_back_wall", [0.0, 4.0, -8.35], [32.0, 8.0, 0.35], [0.105, 0.105, 0.12])

T.position(0.0, 2.55, 8.10).scale(5.0, 5.0, 0.08).rotation(0.0, 3.1416, 0.0) {
    name = "stage_mirror"
    Grabbable {}
    R.cube() {
        Mirror.quality(2048) {}
        Raycastable.enabled()
    }
}

T.position(0.0, 7.10, -7.75) {
    name = "stage_ceiling_truss"
    truss(26)
}

// Four physical studio fixtures cross-light the car. Their common target sits
// just above the stage at the car's initial position.
let car_light_target = [0.0, 1.75, 1.7]
tripod_light(
    "front_left_studio_light",
    [-10.5, 0.14, 4.4],
    car_light_target,
    SL.color(1.0, 0.82, 0.70).intensity(10.0).distance(22.0).angle(0.58).penumbra(0.32),
)
tripod_light(
    "front_right_studio_light",
    [10.5, 0.14, 4.4],
    car_light_target,
    SL.color(0.72, 0.84, 1.0).intensity(10.0).distance(22.0).angle(0.58).penumbra(0.32),
)
tripod_light(
    "rear_left_studio_light",
    [-10.5, 0.14, -5.7],
    car_light_target,
    SL.color(0.72, 0.84, 1.0).intensity(8.0).distance(20.0).angle(0.62).penumbra(0.38),
)
tripod_light(
    "rear_right_studio_light",
    [10.5, 0.14, -5.7],
    car_light_target,
    SL.color(1.0, 0.78, 0.68).intensity(8.0).distance(20.0).angle(0.62).penumbra(0.38),
)

// The camera remains a sibling of the model so it inherits vehicle motion
// while keeping an independently authored cockpit offset and view direction.
I.speed(4.0) {
    name = "car_input"
    InputTransformMode.forward_z() {
        roll_axis_y()
    }

    T.position(-5.0, 0.10, 0.0) {
        name = "car_vehicle"

        T.rotation(0.0, 0.0, 0.0) {
            name = "car_visual"
            GLTF.new("assets/models/car.glb") {
                display_car_anime_material("near")
            }
        }

        // Fixed first-person cockpit view. Camera3D looks along local -Z.
        T.position(0, 3.0, -1.55) {
            name = "car_cockpit_camera"
            C3D { Pointer {} }
        }
    }
}

// A second car is scenery only: it has no Input component and remains parked
// as a lit subject for the driver, studio cameras, and mirror.
T.position(0.0, 0.10, 0).rotation(0.0, -0.32, 0.0) {
    name = "parked_display_car"
    GLTF.new("assets/models/car.glb") {
        display_car_anime_material("middle")
    }
}
