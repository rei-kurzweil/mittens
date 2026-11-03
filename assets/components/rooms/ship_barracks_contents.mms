import { ship_solid, ship_box, ship_label } from "../ship_parts.mms"
import { retail_shelf } from "../retail_shelf.mms"
import { ship_machine_pistol } from "../weapons/ship_machine_pistol.mms"

export fn ship_barracks_contents() {
    return T {
        name = "ship_barracks_contents"
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

// A grabbed weapon can be reparented outside its rack. Retire it explicitly
// along with the room-owned projectile slots when this inventory is collapsed.
export fn ship_clear_barracks_weapons() {
    for index in range(3) {
        let grip=query("#ship_rack_pistol_"+index+"_grip")
        if grip {grip.remove_subtree()}
    }
}
