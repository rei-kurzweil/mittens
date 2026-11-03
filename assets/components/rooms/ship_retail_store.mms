import { ship_box, ship_solid, ship_label, ship_deck, ship_lamp } from "../ship_parts.mms"
import { ship_light_pool } from "../ship_lighting.mms"
import { retail_shelf } from "../retail_shelf.mms"
import { retail_peg_stand } from "../retail_peg_stand.mms"

// Placeholder shop, X=-6..6 and Z=0..6. Open front faces -Z.
export fn ship_retail_store(title, product_color, grocery_stock) {
    return T {
        name = "ship_retail_store"
        ship_solid([0,3.3,3],[12,0.16,6],[0.35,0.39,0.39])
        ship_solid([0.0, 1.6, 6.0], [12.0, 3.2, 0.16], [0.36, 0.41, 0.35])
        for side in [-1.0, 1.0] {
            ship_solid([side * 6.0, 1.6, 3.0], [0.16, 3.2, 6.0], [0.36, 0.41, 0.35])
            ship_light_pool([side * 2.8, 2.7, 2.2], [1.0, 0.86, 0.64], 5.0, 5.0)
        }
        T.position(0.0, 2.65, -0.02).rotation(0.0, Math.pi, 0.0) {
            ship_label([-2.3, 0.0, 0.0], title, 0.25)
        }
        // Counter at the left leaves a broad customer entrance on the right.
        ship_solid([-3.4, 0.52, 1.1], [3.5, 1.04, 0.85], [0.40, 0.32, 0.20])
        ship_solid([-3.4, 1.08, 1.1], [3.7, 0.08, 1.0], [0.66, 0.65, 0.48])
        ship_box([-2.7, 1.30, 1.1], [0.45, 0.35, 0.35], [0.15, 0.23, 0.20])
        ship_lamp([-2.7, 1.35, 0.915], [0.32, 0.16, 0.02], [0.16, 0.63, 0.35])
        let stocks = ["packages", "packages", "packages", "packages", "packages"]
        if grocery_stock { stocks = ["oranges", "bananas", "toilet_paper", "peanut_butter", "packages"] }
        for index in range(5) {
            T.position(-4.5 + index * 2.1, 0.0, 5.55).rotation(0.0, Math.pi, 0.0) { retail_shelf(product_color, stocks[index]) }
        }
        for x in [0.0, 2.1, 4.2] {
            T.position(x, 0.0, 3.0).rotation(0.0, Math.pi, 0.0) { retail_peg_stand() }
        }
    }
}
