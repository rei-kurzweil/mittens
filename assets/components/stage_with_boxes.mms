// The secondary-motion studio floor, contact zone, eight colored boxes,
// and three grabbable tripod lights. Background and fall recovery belong to
// the caller. Keep the root at identity to preserve the authored light targets.
import { tripod_light } from "./tripod_light.mms"

fn static_cube(cube_name, position, size, color) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        name = cube_name
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) }
    }
}

fn grabbable_cube(cube_name, position, size, color) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        name = cube_name
        Grabbable {}
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.8) {} }
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) }
    }
}

export fn stage_with_boxes(stage_name) {
    return T {
        name = stage_name
        tripod_light("studio_key_light", [-4.2, 0.0, 2.8], [0.0, 1.25, 0.0], SL.color(1.0, 0.78, 0.62).intensity(6.0).distance(11.0).angle(0.62).penumbra(0.35))
        tripod_light("studio_fill_light", [4.0, 0.0, 1.4], [0.0, 1.25, 0.0], SL.color(0.48, 0.68, 1.0).intensity(4.5).distance(11.0).angle(0.62).penumbra(0.35))
        tripod_light("studio_rim_light", [1.8, 0.0, -4.2], [0.0, 1.25, 0.0], SL.color(1.0, 0.42, 0.78).intensity(5.0).distance(11.0).angle(0.62).penumbra(0.35))

        // The floor's top face is the world-space y=0 ground plane.
        static_cube("studio_floor", [0.0, -0.05, 0.0], [18.0, 0.1, 18.0], [0.025, 0.035, 0.075])
        // The floor mesh is scaled by its transform; Zone dimensions are local.
        T.position(0.0, -0.05, 0.0).scale(18.0, 0.1, 18.0) {
            name = "studio_floor_contact"
            Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.8) {} }
        }

        // Stable static colliders that can be grabbed and repositioned.
        grabbable_cube("pile_a_base_left",  [-2.8, 0.40, -1.7], [0.85, 0.80, 0.85], [0.95, 0.25, 0.38])
        grabbable_cube("pile_a_base_right", [-1.9, 0.40, -1.7], [0.85, 0.80, 0.85], [1.00, 0.62, 0.15])
        grabbable_cube("pile_a_top",        [-2.35, 1.22, -1.7], [0.85, 0.80, 0.85], [1.00, 0.88, 0.22])

        grabbable_cube("pile_b_base_left",  [1.7, 0.40, -2.8], [0.9, 0.8, 0.9], [0.18, 0.78, 0.96])
        grabbable_cube("pile_b_base_right", [2.65, 0.40, -2.8], [0.9, 0.8, 0.9], [0.28, 0.52, 1.00])
        grabbable_cube("pile_b_top",        [2.18, 1.22, -2.8], [0.9, 0.8, 0.9], [0.62, 0.38, 1.00])

        grabbable_cube("pile_c_base", [3.2, 0.45, 2.5], [1.25, 0.9, 0.9], [0.20, 0.92, 0.62])
        grabbable_cube("pile_c_top",  [3.2, 1.37, 2.5], [0.9, 0.9, 0.9], [0.78, 1.00, 0.34])
    }
}
