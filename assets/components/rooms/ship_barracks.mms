import { ship_store_accordion } from "../ship_store_accordion.mms"
import { ship_barracks_contents, ship_clear_barracks_weapons } from "./ship_barracks_contents.mms"
import { ship_solid, ship_box, ship_deck, ship_label, ship_lamp } from "../ship_parts.mms"
import { ship_light_pool, ship_path_markers } from "../ship_lighting.mms"

// Side room off the bridge hallway. Local X=-8..0, Z=-3..3; door at [0,0,0].
export fn ship_barracks() {
    let wall_color=[0.82,0.75,0.64]
    return T {
        name = "ship_barracks"
        ship_deck([-4.0,0.0],8.0,6.0,3.3,false,[0.87,0.82,0.74])
        ship_solid([-8.0, 1.6, 0.0], [0.16, 3.2, 6.0], wall_color)
        for z in [-3.0, 3.0] {
            ship_solid([-4.0, 1.6, z], [8.0, 3.2, 0.16], wall_color)
        }
        for z in [-2.1, 2.1] {
            ship_solid([0.0, 1.6, z], [0.16, 3.2, 1.8], wall_color)
        }
        ship_solid([0.0, 2.9, 0.0], [0.16, 0.6, 2.4], wall_color)
        ship_light_pool([-4.0, 2.7, 0.0], [1.0,0.92,0.82], 4.5, 6.0)
        for x in [-6.3, -3.6, -1.4] {
            T.position(x,3.05,-0.6).looking_at([x,1.0,-2.45]) {
                name = "ship_barracks_ceiling_spot"
                SL.color(1.0,0.91,0.80).intensity(12.0).distance(7.0).angle(0.85).penumbra(0.35)
            }
            ship_lamp([x,3.13,-0.6], [0.30,0.06,0.24], [1.0,0.90,0.77])
        }
        ship_path_markers([-7.0, 0.05, 2.7], [1.0, 0.0], 5, 1.5, [0.22, 0.59, 0.55])
        ship_store_accordion("ship_barracks_inventory","GROCERY / SECURITY SUPPLIES",
            ship_barracks_contents,null,null,[-0.15,2.7,1.0],true,Math.pi/2.0,ship_clear_barracks_weapons)

    }
}
