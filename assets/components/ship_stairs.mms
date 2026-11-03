import { ship_box, ship_solid } from "./ship_parts.mms"

// Ascends local +Z. Origin is the foot of the stairs, top is [0,rise,run].
// Fine axis-aligned collision treads let the capsule round over the rise.
// Pitched box zones are not supported by the current static contact solver.
export fn ship_stairs(width, rise, run, count) {
    let angle = -Math.atan2(rise, run)
    let slope_length = Math.sqrt(rise * rise + run * run)
    return T {
        name = "ship_stairs"
        CombineMesh {
            for step in range(count) {
                let height = (step + 0.5) * rise / count
                ship_box([0.0, height / 2.0, (step + 0.5) * run / count],
                    [width, height, run / count], [0.48, 0.53, 0.48])
                ship_box([0.0, height + 0.003, (step + 0.08) * run / count],
                    [width, 0.006, 0.035], [0.85, 0.72, 0.26])
            }
        }
        // Five 3 cm collision rises per visible 15 cm tread. All geometry and
        // zones are built once; collision queries use the static broad phase.
        for step in range(count * 5) {
            let height = (step + 1) * rise / (count * 5)
            T.position(0.0, height / 2.0, (step + 0.5) * run / (count * 5))
                .scale(width, height, run / (count * 5)) {
                Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.8) {} }
            }
        }
        for step in range(count) {
            for side in [-1.0, 1.0] {
                ship_solid([side * (width / 2.0 + 0.04),
                    (step + 0.5) * rise / count + 0.55, (step + 0.5) * run / count],
                    [0.08, 1.10, run / count], [0.25, 0.34, 0.31])
            }
        }
        T.position(0.0, rise / 2.0, run / 2.0).rotation(angle, 0.0, 0.0) {
            for side in [-1.0, 1.0] {
                ship_box([side * (width / 2.0 + 0.04), 1.1, 0.0],
                    [0.10, 0.08, slope_length], [0.48, 0.57, 0.51])
            }
        }
    }
}
