import { ship_motorhome_group, ship_driving_vehicle_group } from "../vehicles/ship_vehicle_groups.mms"
import { ship_store_accordion } from "../ship_store_accordion.mms"
import { ship_biodome } from "../ship_biodome.mms"
import { ship_box, ship_solid, ship_deck, ship_label, ship_lamp } from "../ship_parts.mms"
import { ship_path_markers, ship_light_pool } from "../ship_lighting.mms"
import { ship_convenience_store } from "./ship_convenience_store.mms"
import { ship_liquor_store } from "./ship_liquor_store.mms"

// 96 x 144 m rectangular vehicle enclosure; west entrance [-48,0,-1].
export fn ship_trailer_park() {
    return T {
        name = "ship_trailer_park"
        // All vehicles and stores stay in a compact neighbourhood around the
        // original lot. The much larger enclosure leaves room for driving.
        ship_solid([0,-0.1,-1], [96,0.2,144], [0.19,0.23,0.25])
        T.position(0,0,-1) { ship_biodome("ship_vehicle_biodome",96,144,9.0,0.0,true,false) }
        ship_box([-24,0.012,-1], [48,0.024,2.0], [0.37,0.38,0.30])
        ship_path_markers([-46,0.04,0.0], [1,0], 12,3.8,[0.64,0.49,0.22])
        ship_box([0.0, 0.01, -1.0], [24.0, 0.02, 2.0], [0.37, 0.38, 0.30])
        for z in [-2.1, 0.1] {
            ship_path_markers([-10.5, 0.04, z], [1.0, 0.0], 12, 1.9, [0.64, 0.49, 0.22])
        }
        ship_store_accordion("ship_motorhomes","MOTORHOMES",ship_motorhome_group,
            null,null,[-7,2.8,-10.2],false)
        ship_store_accordion("ship_driving_vehicles","PICKUPS / MOTORCYCLES",ship_driving_vehicle_group,
            null,null,[1,2.8,-10.2],false)
        for x in [-8.0, 8.0] {
            ship_lamp([x, 4.55, -1.0], [1.2, 0.06, 0.30], [0.70, 0.63, 0.42])
            ship_light_pool([x, 3.8, -1.0], [0.84, 0.75, 0.53], 2.0, 6.0)
        }
        // Service walkway remains open between the parked vehicles and shops.
        ship_box([0.0, 0.015, 9.4], [24.0, 0.03, 1.2], [0.48, 0.47, 0.35])
        T.position(-6,0,10) { ship_store_accordion("ship_convenience_shop","DECKSIDE CONVENIENCE",ship_convenience_store,12,6,null,false) }
        T.position(6,0,10) { ship_store_accordion("ship_liquor_shop","ORBITAL LIQUOR",ship_liquor_store,12,6,null,false) }
        T.position(-47.8, 2.0, -1.0).rotation(0.0, -Math.pi / 2.0, 0.0) {
            ship_label([-0.9, 0.0, 0.0], "DECKSIDE TRAILER PARK", 0.15)
        }
    }
}
