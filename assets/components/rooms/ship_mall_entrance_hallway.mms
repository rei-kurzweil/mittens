import { ship_solid, ship_label, ship_deck } from "../ship_parts.mms"
import { ship_path_markers, ship_light_pool } from "../ship_lighting.mms"

// Bridge-to-cafeteria hall only. The washroom branches off the cafeteria.
export fn ship_mall_entrance_hallway() {
    return T {
        name = "ship_mall_entrance_hallway"
        ship_deck([0.0, 3.0], 3.0, 6.0, 3.3, false)
        ship_path_markers([-1.37, 0.14, 0.6], [0.0, 1.0], 4, 1.5, [0.16, 0.58, 0.78])
        ship_path_markers([1.1, 0.025, 0.6], [0.0, 1.0], 4, 1.5, [0.16, 0.58, 0.78])
        ship_light_pool([0.0, 0.4, 3.0], [0.12, 0.42, 0.65], 0.40, 2.5)
        for z in [0.9, 5.1] {
            ship_solid([-1.5, 1.6, z], [0.16, 3.2, 1.8], [0.40, 0.46, 0.46])
        }
        ship_solid([-1.5, 2.9, 3.0], [0.16, 0.6, 2.4], [0.40, 0.46, 0.46])
        T.position(-1.38, 2.65, 3.0).rotation(0.0, Math.pi/2.0, 0.0) {
            ship_label([-0.6, 0.0, 0.0], "GROCERY / SECURITY SUPPLIES", 0.12)
        }
        ship_solid([1.5, 1.6, 3.0], [0.16, 3.2, 6.0], [0.40, 0.46, 0.46])
        T.position(0.0, 2.55, 5.9).rotation(0.0, Math.pi, 0.0) {
            ship_label([-0.9, 0.0, 0.0], "CAFETERIA / COFFEE", 0.15)
        }
    }
}
