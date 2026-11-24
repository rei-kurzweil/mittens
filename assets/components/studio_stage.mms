import { truss } from "truss.mms"
import { studio_environment_material } from "./materials/studio_anime.mms"

// A destroyable surface splits into at most 512 chunks; `chunk` (metres)
// keeps large boxes such as the floor under that.
fn stage_box(box_name, position, size, color, material, destroyable, chunk) {
    if chunk == null { chunk = 1.0 }
    return T.position(position[0], position[1], position[2])
        .scale(size[0], size[1], size[2]) {
        name = box_name
        if destroyable { Destroyable.threshold(3.0).chunk_size(chunk) }
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) material }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.8) {} }
    }
}

// The Mittens Corp deck, steps, back wall, and overhead truss. The studio
// floor, mirror, lights, and suspended walkway belong to the calling scene.
// `material` (optional) replaces the far-bucket studio Anime material, e.g.
// with a custom definition for the Android renderer. `destroyable` (optional)
// lets damage cut the deck, steps and back wall.
export fn studio_stage(stage_name, material, destroyable) {
    if !material { material = studio_environment_material() }
    return T {
        name = stage_name
        stage_box("stage_deck",       [0.0,  0.00, -1.5], [32.0, 0.24, 14.0], [0.18, 0.18, 0.20], material, destroyable)
        stage_box("stage_upper_step", [0.0, -0.24,  5.7], [32.0, 0.28,  0.8], [0.14, 0.14, 0.16], material, destroyable)
        stage_box("stage_lower_step", [0.0, -0.56,  6.3], [32.0, 0.36,  0.8], [0.10, 0.10, 0.12], material, destroyable)
        stage_box("stage_back_wall",  [0.0,  4.00, -8.35], [32.0, 8.00, 0.35], [0.105, 0.105, 0.12], material, destroyable)

        T.position(0.0, 7.10, -7.75) {
            name = "stage_ceiling_truss"
            truss(26)
        }
    }
}

// Matching floor for the Corp studio; top surface is y=-0.85.
export fn studio_floor(destroyable) {
    return stage_box("studio_floor", [0.0, -0.92, 1.0], [54.0, 0.14, 32.0], [0.035, 0.037, 0.043], null, destroyable, 2.0)
}
