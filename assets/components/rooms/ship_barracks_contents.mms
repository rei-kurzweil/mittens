import { ship_solid, ship_box, ship_label } from "../ship_parts.mms"
import { retail_peg_stand } from "../retail_peg_stand.mms"
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
        ship_label([-6.5, 2.2, -2.35], "SECURITY / MACHINE PISTOLS", 0.14)
        T.position(-3.6, 0.0, -2.55) { retail_shelf(null, "toilet_paper") }
        T.position(-1.5, 0.0, -2.55) { retail_shelf(null, "peanut_butter") }
        // Produce along the opposite wall faces the open central shopping aisle.
        T.position(-6.2,0,2.55).rotation(0,Math.pi,0) {
            name="ship_grocery_bananas"
            retail_shelf(null,"bananas")
        }
        T.position(-4.1,0,2.55).rotation(0,Math.pi,0) {
            name="ship_grocery_oranges"
            retail_shelf(null,"oranges")
        }
        T.position(-7.7,0,0.6).rotation(0,Math.pi/2.0,0) {retail_peg_stand()}
        T.position(-5.2,2.2,2.25).rotation(0,Math.pi,0) {
            ship_label([-1.2,0,0],"FRESH FRUIT / BANANAS",0.12)
        }
        // Small checkout leaves the doorway and middle aisle open.
        T {name="ship_grocery_checkout"
            ship_solid([-1.5,0.50,1.9],[1.85,1.0,0.75],[0.40,0.32,0.20])
            ship_solid([-1.5,1.04,1.9],[2.0,0.08,0.85],[0.66,0.65,0.48])
            ship_box([-1.15,1.25,1.9],[0.40,0.34,0.32],[0.15,0.23,0.20])
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
