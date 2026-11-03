import { ship_box, ship_solid } from "./ship_parts.mms"

// Small freestanding pegboard, with a 4 x 4 grid of hanging packages.
export fn retail_peg_stand() {
    return T {
        name = "retail_peg_stand"
        ship_solid([0.0, 0.06, 0.0], [1.15, 0.12, 0.65], [0.26, 0.33, 0.30])
        ship_solid([0.0, 1.10, -0.12], [1.05, 1.8, 0.07], [0.58, 0.55, 0.40])
        CombineMesh {
            for row in range(4) {
                for column in range(4) {
                    let x = -0.39 + column * 0.26
                    let y = 0.48 + row * 0.40
                    ship_box([x, y + 0.12, 0.02], [0.025, 0.025, 0.25], [0.55, 0.61, 0.56])
                    ship_box([x, y, 0.12], [0.19, 0.25, 0.035], [0.65, 0.38, 0.19])
                    ship_box([x, y + 0.05, 0.145], [0.12, 0.06, 0.012], [0.85, 0.80, 0.57])
                }
            }
        }
    }
}
