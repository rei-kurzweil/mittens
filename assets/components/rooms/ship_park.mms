import { ship_biodome } from "../ship_biodome.mms"
import { ship_box, ship_solid, ship_label } from "../ship_parts.mms"
import { sakura_tree } from "../plants/sakura_tree.mms"
import { ship_path_markers, ship_light_pool } from "../ship_lighting.mms"
import { ship_park_ground, park_ground_height } from "../floors/ship_park_ground.mms"

// 64 x 48 m rectangular park enclosure; floor at local Y=0.
// Entrance faces -X, lining up with the cafeteria's upper landing.

export fn ship_park() {
    return T {
        name = "ship_park"
        ship_solid([0.0, -0.10, 0.0], [64.0, 0.20, 48.0], [0.28, 0.39, 0.24])
        ship_park_ground(4)
        ship_biodome("ship_park_glass_dome", 64.0, 48.0, 7.0, 0.0, true, true)
        for x in [-18.0,0.0,18.0] {
            ship_light_pool([x, 3.8, 0.0], [0.88, 0.79, 0.66], 4.0, 15.0)
        }
        for side in [-1.0, 1.0] {
            ship_path_markers([-30.0, 0.04, side * 0.95], [1.0, 0.0], 21, 3.0, [0.38, 0.72, 0.45])
        }
        // Main path continues the landing; the crossing path reaches the grove.
        ship_box([0.0, 0.005, 0.0], [64.0, 0.01, 2.0], [0.63, 0.60, 0.44])
        ship_box([0.0, 0.006, 0.0], [2.0, 0.012, 46.0], [0.63, 0.60, 0.44])
        for x in [-24.0,-12.0,0.0,12.0,24.0] {
            for z in [-16.0,-8.0,8.0,16.0] {
                T.position(x, park_ground_height(x,z,4), z) { sakura_tree(2, 1.15, 0.20) }
            }
        }
        for z in [-2.0, 2.0] {
            ship_solid([0.0, 0.48, z], [2.2, 0.12, 0.55], [0.45, 0.34, 0.22])
            for x in [-0.85, 0.85] {
                ship_box([x, 0.21, z], [0.12, 0.42, 0.44], [0.26, 0.35, 0.30])
            }
        }
        // East exit continues into the larger rectangular vehicle enclosure.
        T { name = "ship_biodome_connector"
            ship_solid([33.0,-0.1,0.0], [2.0,0.2,2.4], [0.28,0.39,0.24])
            for z in [-1.2,1.2] {
                ship_solid([33.0,0.55,z], [2.0,1.10,0.10], [0.25,0.39,0.35])
            }
            ship_path_markers([32.3,0.04,0.85], [1,0], 3, 0.6, [0.38,0.72,0.45])
        }
        T.position(-30.2, 2.1, 0.0).rotation(0.0, -Math.pi / 2.0, 0.0) {
            ship_label([-0.8, 0.0, 0.0], "CHERRY GROVE", 0.19)
        }
    }
}
