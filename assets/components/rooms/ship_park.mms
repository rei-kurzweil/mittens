import { ship_biodome } from "../ship_biodome.mms"
import { ship_box, ship_solid, ship_label } from "../ship_parts.mms"
import { sakura_tree } from "../plants/sakura_tree.mms"
import { ship_path_markers, ship_light_pool } from "../ship_lighting.mms"
import { ship_park_ground, park_ground_height } from "../floors/ship_park_ground.mms"

// 16 x 12 m park, centred at local origin; floor at local Y=0.
// Elliptical glass dome has a 2.4 m drum and a 4 m crown above it.
// Entrance faces -X, lining up with the cafeteria's upper landing.

export fn ship_park() {
    return T {
        name = "ship_park"
        ship_solid([0.0, -0.10, 0.0], [16.0, 0.20, 12.0], [0.28, 0.39, 0.24])
        ship_park_ground()
        ship_biodome("ship_park_glass_dome", 16.0, 12.0, 2.4, 4.0, 12, 4)
        for x in [-3.5, 3.5] {
            ship_light_pool([x, 3.8, 0.0], [0.88, 0.79, 0.66], 3.0, 6.5)
        }
        for side in [-1.0, 1.0] {
            ship_path_markers([-6.5, 0.04, side * 0.95], [1.0, 0.0], 7, 2.0, [0.38, 0.72, 0.45])
        }
        // Main path continues the landing; the crossing path reaches the grove.
        ship_box([0.0, 0.005, 0.0], [16.0, 0.01, 1.6], [0.63, 0.60, 0.44])
        ship_box([0.0, 0.006, 0.0], [1.6, 0.012, 9.0], [0.63, 0.60, 0.44])
        for x in [-3.8, 3.8] {
            for z in [-2.8, 2.8] {
                T.position(x, park_ground_height(x, z), z) { sakura_tree(2, 1.15, 0.20) }
            }
        }
        for z in [-2.0, 2.0] {
            ship_solid([0.0, 0.48, z], [2.2, 0.12, 0.55], [0.45, 0.34, 0.22])
            for x in [-0.85, 0.85] {
                ship_box([x, 0.21, z], [0.12, 0.42, 0.44], [0.26, 0.35, 0.30])
            }
        }
        // Outside the dome's oval, the deck corners remain guarded balconies.
        for z in [-5.95, 5.95] {
            ship_solid([0.0, 0.55, z], [16.0, 1.10, 0.10], [0.25, 0.39, 0.35])
        }
        // East exit continues into the larger vehicle biodome.
        for z in [-3.6,3.6] {
            ship_solid([7.95,0.55,z], [0.10,1.10,4.8], [0.25,0.39,0.35])
        }
        T { name = "ship_biodome_connector"
            ship_solid([9.0,-0.1,0.0], [2.0,0.2,2.4], [0.28,0.39,0.24])
            for z in [-1.2,1.2] {
                ship_solid([9.0,0.55,z], [2.0,1.10,0.10], [0.25,0.39,0.35])
            }
            ship_path_markers([8.3,0.04,0.85], [1,0], 3, 0.6, [0.38,0.72,0.45])
        }
        for z in [-3.45, 3.45] {
            ship_solid([-7.95, 0.55, z], [0.10, 1.10, 5.0], [0.25, 0.39, 0.35])
        }
        T.position(-6.2, 2.1, 0.0).rotation(0.0, -Math.pi / 2.0, 0.0) {
            ship_label([-0.8, 0.0, 0.0], "CHERRY GROVE", 0.19)
        }
    }
}
