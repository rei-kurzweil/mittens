import { ship_solid, ship_box, ship_label, ship_deck } from "../ship_parts.mms"
import { ship_light_pool } from "../ship_lighting.mms"

// Local X=-3..3, Z=0..5. Open storefront at Z=0, serving face toward -Z.
export fn ship_mall_coffeeshop() {
    return T {
        name = "ship_mall_coffeeshop"
        ship_deck([0.0, 2.5], 6.0, 5.0)
        ship_light_pool([0.0, 2.7, 1.2], [1.0, 0.80, 0.52], 7.0, 5.0)
        ship_light_pool([0.0, 2.4, -0.7], [1.0, 0.79, 0.50], 4.0, 3.0)
        ship_solid([0.0, 1.6, 5.0], [6.0, 3.2, 0.16], [0.34, 0.31, 0.25])
        for side in [-1.0, 1.0] {
            ship_solid([side * 3.0, 1.6, 2.5], [0.16, 3.2, 5.0], [0.40, 0.43, 0.38])
        }
        // Side access for staff remains open beside the counter.
        ship_solid([-0.5, 0.50, 1.2], [4.3, 1.0, 0.75], [0.39, 0.25, 0.14])
        ship_solid([-0.5, 1.04, 1.2], [4.5, 0.08, 0.90], [0.70, 0.66, 0.50])
        ship_box([-1.6, 1.35, 1.2], [0.75, 0.54, 0.50], [0.48, 0.54, 0.51])
        for x in [-1.8, -1.5] {
            ship_box([x, 1.12, 0.96], [0.13, 0.16, 0.13], [0.89, 0.86, 0.72])
        }
        ship_box([0.5, 1.29, 1.2], [1.2, 0.42, 0.55], [0.27, 0.36, 0.31])
        T.position(0.0, 2.5, 0.0).rotation(0.0, 3.1415927, 0.0) {
            ship_label([-1.2, 0.0, 0.0], "ORBIT COFFEE", 0.24)
        }
    }
}
