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
        T.position(i*1.1, 0.0, j*1.1) {
            R.cube() {
                C.rgba(r, 0.4, 0.8, 1.0)
            }
        }
    }
}

// light rig
let light_rig = T {
    T.position(5, -5, 0) {
        DL {
            intensity(1.0)
            C.rgba(1.0, 0.95, 0.1, 1.0)
        }
    }

    T.position(-5, 5, 0) {
        DL {
            intensity(0.7)
            C.rgba(0.1, 0.95, 1.0, 1.0)
        }
    }

    T.position(0, 5, 5) {
        DL {
            intensity(0.8)
            C.rgba(1.0, 0.1, 0.95, 1.0)
        }
    }
}

light_rig

AL {
    C.rgba(0.1,0.2,0.2, 1.0)
}


for y in range(128) {
    for x in range(128) {
        let r = x / 128.0
        let g = y / 128.0
        let b = 0.5

        let x2 = x - 64.0
        let y2 = y - 64.0

        T.position(x2*1.1, -2.0, y2*1.1) {
            R.cube() {
                C.rgba(r, g, b, 1.0)
            }
        }
    } 
}