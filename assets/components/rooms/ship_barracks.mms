import { ship_solid, ship_box, ship_deck, ship_label, ship_lamp } from "../ship_parts.mms"
import { ship_light_pool, ship_path_markers } from "../ship_lighting.mms"
import { retail_shelf } from "../retail_shelf.mms"
import { ship_machine_pistol } from "../weapons/ship_machine_pistol.mms"

// Side room off the bridge hallway. Local X=-8..0, Z=-3..3; door at [0,0,0].
export fn ship_barracks() {
    return T {
        name = "ship_barracks"
        ship_deck([-4.0, 0.0], 8.0, 6.0, 3.3, false)
        ship_solid([-8.0, 1.6, 0.0], [0.16, 3.2, 6.0], [0.31, 0.38, 0.35])
        for z in [-3.0, 3.0] {
            ship_solid([-4.0, 1.6, z], [8.0, 3.2, 0.16], [0.31, 0.38, 0.35])
        }
        for z in [-2.1, 2.1] {
            ship_solid([0.0, 1.6, z], [0.16, 3.2, 1.8], [0.31, 0.38, 0.35])
        }
        ship_solid([0.0, 2.9, 0.0], [0.16, 0.6, 2.4], [0.31, 0.38, 0.35])
        ship_light_pool([-4.0, 2.7, 0.0], [0.76, 0.85, 0.77], 4.5, 6.0)
        for x in [-6.3, -3.6, -1.4] {
            T.position(x,3.05,-0.6).looking_at([x,1.0,-2.45]) {
                name = "ship_barracks_ceiling_spot"
                SL.color(0.86,0.92,1.0).intensity(12.0).distance(7.0).angle(0.85).penumbra(0.35)
            }
            ship_lamp([x,3.13,-0.6], [0.30,0.06,0.24], [0.76,0.87,1.0])
        }
        ship_path_markers([-7.0, 0.05, 2.7], [1.0, 0.0], 5, 1.5, [0.22, 0.59, 0.55])
        // Same supermarket fixture as the shops; the gun rack uses empty stock.
        T.position(-5.7, 0.0, -2.55) { retail_shelf([0.3, 0.4, 0.3], "empty") }
        for index in range(3) {
            let x = -6.25 + index * 0.55
            T.position(x, 1.15, -2.40) { ship_machine_pistol("ship_rack_pistol_" + index) }
            ship_box([x, 1.08, -2.35], [0.38, 0.035, 0.24], [0.12, 0.17, 0.14])
        }
        ship_label([-6.5, 2.2, -2.35], "MACHINE PISTOLS", 0.14)
        T.position(-3.6, 0.0, -2.55) { retail_shelf(null, "toilet_paper") }
        T.position(-1.5, 0.0, -2.55) { retail_shelf(null, "peanut_butter") }
        // Two simple bunks along the opposite wall, with an open centre aisle.
        for x in [-6.3, -3.4] {
            for y in [0.5, 1.8] {
                ship_solid([x, y, 1.9], [2.2, 0.12, 1.1], [0.26, 0.34, 0.29])
                ship_box([x, y + 0.11, 1.9], [2.1, 0.10, 1.0], [0.48, 0.52, 0.35])
            }
        }
    }
}
