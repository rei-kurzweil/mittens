import { ship_solid, ship_box, ship_lamp } from "./ship_parts.mms"

// Two side-by-side banks of drawer-like server modules in one desk-height cube.
export fn computer_cabinet() {
    return T {
        name = "computer_cabinet"
        ship_solid([0.0, 0.42, 0.0], [1.2, 0.84, 0.85], [0.39, 0.43, 0.40])
        for side in [-1.0, 1.0] {
            for row in range(5) {
                let y = 0.13 + row * 0.145
                ship_box([side * 0.29, y, 0.433], [0.53, 0.125, 0.025], [0.13, 0.18, 0.18])
                ship_box([side * 0.29, y + 0.025, 0.455], [0.24, 0.018, 0.025], [0.54, 0.58, 0.53])
                ship_lamp([side * 0.29 - 0.18, y - 0.025, 0.454], [0.035, 0.022, 0.012], [0.25, 0.95, 0.20])
                ship_lamp([side * 0.29 - 0.12, y - 0.025, 0.454], [0.035, 0.022, 0.012], [1.0, 0.74, 0.12])
            }
        }
    }
}
