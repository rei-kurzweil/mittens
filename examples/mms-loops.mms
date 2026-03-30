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
                NV {}
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
            intensity(1.0)
            C.rgba(0.1, 0.95, 1.0, 1.0)
        }
    }

    T.position(0, 5, 5) {
        DL {
            intensity(1.0)
            C.rgba(1.0, 0.1, 0.95, 1.0)
        }
    }
}

light_rig

BGC {
    C.rgba(0.9, 0.9, 0.9, 1.0)
}
AL {
    C.rgba(0.2,0.2,0.2, 1.0)
}


for z in range(128) {
    for x in range(128) {
        let r = 0.1 + (x % 32 / 32.0)
        let g = 0.1 + (z % 32 / 32.0)
        
        let x2 = x - 64.0
        let z2 = z - 64.0

        let y = -2.0;

        if (z > 64 || x > 64) {
            y = 2.0 + ((x + z) % 16);
        }

        if (z > 96 || x > 96) {
            y = 2.0 + ((x + z) % 32);
        }

        let b = 0.1 + ((x + z + y) % 16 / 16.0)

        T.position(x2, y, z2) {
            R.cube() {
                C.rgba(r, g, b, 1.0)
            }
        }
    } 
}