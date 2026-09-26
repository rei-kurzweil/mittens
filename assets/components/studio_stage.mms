import { truss } from "truss.mms"

fn stage_box(box_name, position, size, color) {
    return T.position(position[0], position[1], position[2])
        .scale(size[0], size[1], size[2]) {
        name = box_name
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) }
    }
}

// The Mittens Corp deck, steps, back wall, and overhead truss. The studio
// floor, mirror, lights, and suspended walkway belong to the calling scene.
export fn studio_stage(stage_name) {
    return T {
        name = stage_name
        stage_box("stage_deck",       [0.0,  0.00, -1.5], [32.0, 0.24, 14.0], [0.18, 0.18, 0.20])
        stage_box("stage_upper_step", [0.0, -0.24,  5.7], [32.0, 0.28,  0.8], [0.14, 0.14, 0.16])
        stage_box("stage_lower_step", [0.0, -0.56,  6.3], [32.0, 0.36,  0.8], [0.10, 0.10, 0.12])
        stage_box("stage_back_wall",  [0.0,  4.00, -8.35], [32.0, 8.00, 0.35], [0.105, 0.105, 0.12])

        T.position(0.0, 7.10, -7.75) {
            name = "stage_ceiling_truss"
            truss(26)
        }
    }
}
