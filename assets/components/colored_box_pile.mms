// Repositionable static supports, following stage_with_boxes.mms. Each cube
// has matching render/contact extents and keeps its pose after release.
fn pile_box(box_name, position, color) {
    return T.position(position[0], position[1], position[2]) {
        name = box_name
        Grabbable {}
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.8) {} }
    }
}

// Position is the bottom-center of a two-base, one-top pile of 1 m cubes.
// Use a unique pile_name so individual boxes can be queried across piles.
export fn colored_box_pile(pile_name, position) {
    return T.position(position[0], position[1], position[2]) {
        name = pile_name
        pile_box(pile_name + "_red_base", [-0.5, 0.5, 0.0], [0.95, 0.62, 0.65])
        pile_box(pile_name + "_green_base", [0.5, 0.5, 0.0], [0.66, 0.90, 0.68])
        pile_box(pile_name + "_blue_top", [0.0, 1.5, 0.0], [0.67, 0.80, 0.97])
    }
}
