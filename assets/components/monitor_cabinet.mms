import { ship_solid, ship_box, ship_lamp } from "./ship_parts.mms"
import { crt_monitor } from "./crt_monitor.mms"

export fn monitor_cabinet() {
    return T {
        name = "monitor_cabinet"
        // Open knee space beneath the tilted control surface.
        for side in [-1.0, 1.0] {
            ship_solid([side * 0.52, 0.40, -0.10], [0.12, 0.80, 0.65], [0.39, 0.43, 0.40])
        }
        ship_solid([0.0, 0.73, -0.28], [1.16, 0.12, 0.32], [0.43, 0.48, 0.42])
        T.position(0.0, 0.87, 0.10).rotation(0.7853982, 0.0, 0.0) {
            ship_solid([0.0, 0.0, 0.0], [1.16, 0.08, 0.52], [0.22, 0.29, 0.27])
            for row in range(3) {
                for key in range(8) {
                    ship_lamp([-0.42 + key * 0.12, 0.045, -0.15 + row * 0.13], [0.07, 0.014, 0.07], [0.34 + row * 0.18, 0.66, 0.22])
                }
            }
        }
        T.position(0.0, 0.80, -0.28) { crt_monitor() }
    }
}
