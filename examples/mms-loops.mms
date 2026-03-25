// Phase 5 demo: procedural spawning with for/in and range()
//
// Spawns a 4×4 grid of cubes, coloured by position, with a gap in the middle
// (where both i and j are 1 or 2) — exercising break/continue.

fn grid_color(i, j) {
    if i == 0.0 { return 1.0 }
    if j == 0.0 { return 0.5 }
    return 0.2
}

for i in range(4) {
    for j in range(4) {
        // skip a 2×2 hole in the centre of the grid
        if i == 1.0 { if j == 1.0 { continue } }
        if i == 1.0 { if j == 2.0 { continue } }
        if i == 2.0 { if j == 1.0 { continue } }
        if i == 2.0 { if j == 2.0 { continue } }

        let r = grid_color(i, j)
        T.position(i, 0.0, j) {
            R.cube() {
                C.rgba(r, 0.4, 0.8, 1.0)
            }
        }
    }
}
