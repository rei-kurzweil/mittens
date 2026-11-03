import { ship_solid, ship_box, ship_lamp } from "./ship_parts.mms"

// Rings and three spokes leave three real openings plus the spindle hole.
// A simple R.polygon has no holes; this avoids CSG and custom mesh support.
fn reel() {
    return T {
        for z in [-0.025, 0.025] {
            T.position(0.0, 0.0, z) {
                R.partial_annulus_2d(0.18, 0.23, 0.0, 6.2831853, 32) { C.rgba(0.72, 0.76, 0.72, 1.0) }
                R.partial_annulus_2d(0.035, 0.075, 0.0, 6.2831853, 24) { C.rgba(0.72, 0.76, 0.72, 1.0) }
                for spoke in range(3) {
                    T.rotation(0.0, 0.0, spoke * 2.0943951) {
                        ship_box([0.125, 0.0, 0.0], [0.13, 0.045, 0.008], [0.72, 0.76, 0.72])
                    }
                }
            }
        }
        R.partial_annulus_2d(0.075, 0.16, 0.0, 6.2831853, 32) { C.rgba(0.13, 0.075, 0.035, 1.0) }
    }
}

export fn tape_cabinet() {
    return T {
        name = "tape_cabinet"
        ship_solid([0.0, 0.42, 0.0], [1.2, 0.84, 0.85], [0.49, 0.53, 0.44])
        ship_box([0.0, 0.47, 0.435], [1.10, 0.63, 0.025], [0.055, 0.075, 0.07])
        for side in [-1.0, 1.0] {
            T.position(side * 0.29, 0.48, 0.50) { reel() }
        }
        ship_box([0.0, 0.32, 0.50], [0.59, 0.018, 0.012], [0.22, 0.12, 0.045])
        ship_box([0.0, 0.32, 0.49], [0.10, 0.08, 0.06], [0.48, 0.53, 0.50])
        ship_lamp([-0.43, 0.09, 0.445], [0.08, 0.03, 0.02], [0.35, 0.90, 0.20])
    }
}
