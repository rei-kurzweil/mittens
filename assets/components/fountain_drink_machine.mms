import { ship_box, ship_lamp } from "./ship_parts.mms"

// Four independent visible taps; front faces +Z. Place on a counter.
export fn fountain_drink_machine() {
    let flavors = ["COLA", "CITRUS", "ORANGE", "GRAPE"]
    let colors = [[0.56, 0.10, 0.08], [0.46, 0.73, 0.12], [0.95, 0.38, 0.04], [0.45, 0.18, 0.66]]
    return T {
        name = "fountain_drink_machine"
        CombineMesh {
            Material(MaterialDefinition.anime())
            ship_box([0.0, 0.55, -0.04], [1.06, 1.10, 0.48], [0.24, 0.29, 0.27])
            ship_box([0.0, 0.38, 0.22], [0.95, 0.58, 0.025], [0.035, 0.055, 0.045])
            ship_box([0.0, 0.06, 0.30], [1.10, 0.08, 0.30], [0.46, 0.51, 0.47])
            for grate in range(12) {
                ship_box([-0.48+grate*0.087, 0.105, 0.30], [0.04, 0.012, 0.24], [0.12, 0.17, 0.15])
            }
        }
        for index in range(4) {
            let x = -0.375+index*0.25
            let color = colors[index]
            T.position(x, 0.0, 0.0) {
                name = "fountain_drink_nozzle_" + index
                ship_lamp([0.0, 0.85, 0.21], [0.22, 0.32, 0.035], color)
                T.position(-0.095, 0.89, 0.235).scale(0.027, 0.027, 0.027) {
                    Text { flavors[index] C.rgba(1.0, 0.96, 0.85, 1.0) Unlit {} }
                }
                ship_box([0.0, 0.58, 0.265], [0.045, 0.13, 0.07], color)
                T.position(0.0, 0.40, 0.30) {
                    R.cylinder(0.023, 0.10, 12) { C.rgba(0.56, 0.60, 0.54, 1.0) }
                }
            }
        }
    }
}
