import { pickup_truck } from "../vehicles/pickup_truck.mms"
// import { ship_store_accordion } from "../ship_store_accordion.mms"
import { ship_biodome } from "../ship_biodome.mms"
import { ship_box, ship_solid, ship_label } from "../ship_parts.mms"
// import { sakura_tree } from "../plants/sakura_tree.mms"
import { ship_path_markers, ship_light_pool } from "../ship_lighting.mms"
import { ship_park_ground, park_ground_height } from "../floors/ship_park_ground.mms"

// Tree geometry and trunk collision live together in removable orchard halves.
// fn orchard_rows(rows) {
//     return T {
//         for x in [-24.0,-12.0,0.0,12.0,24.0] {
//             for z in rows {
//                 T.position(x,park_ground_height(x,z,4),z) {
//                     name="ship_orchard_tree"
//                     sakura_tree(2,1.15,0.20)
//                 }
//             }
//         }
//     }
// }

// 64 x 48 m rectangular park enclosure; floor at local Y=0.
// Entrance faces -X, lining up with the cafeteria's upper landing.

export fn ship_park() {
    return T {
        name = "ship_park"
        ship_solid([0.0, -0.10, 0.0], [64.0, 0.20, 48.0], [0.28, 0.39, 0.24])
        ship_park_ground(4)
        ship_biodome("ship_park_glass_dome", 64.0, 48.0, 7.0, 0.0, true, false)
        for x in [-18.0,0.0,18.0] {
            ship_light_pool([x, 3.8, 0.0], [0.88, 0.79, 0.66], 4.0, 15.0)
        }
        for side in [-1.0, 1.0] {
            ship_path_markers([-30.0, 0.04, side * 0.95], [1.0, 0.0], 21, 3.0, [0.38, 0.72, 0.45])
        }
        // Main path continues the landing; the crossing path reaches the grove.
        ship_box([0.0, 0.005, 0.0], [64.0, 0.01, 2.0], [0.63, 0.60, 0.44])
        ship_box([0.0, 0.006, 0.0], [2.0, 0.012, 46.0], [0.63, 0.60, 0.44])
        // Controls beside the west entrance leave the central path clear.
        // Ground, benches and the enclosure stay loaded when trees are hidden.
        // PERF ISOLATION: restore this block to instantiate its accordion and content.
        // ship_store_accordion("ship_orchard_north","ORCHARD / NORTH TREES",fn() {
        //     return orchard_rows([-16.0,-8.0])
        // },null,null,[-29.0,2.35,-3.4],true,-Math.pi/2.0)
        // ship_store_accordion("ship_orchard_south","ORCHARD / SOUTH TREES",fn() {
        //     return orchard_rows([8.0,16.0])
        // },null,null,[-29.0,2.35,1.4],true,-Math.pi/2.0)
        for z in [-2.0, 2.0] {
            ship_solid([0.0, 0.48, z], [2.2, 0.12, 0.55], [0.45, 0.34, 0.22])
            for x in [-0.85, 0.85] {
                ship_box([x, 0.21, z], [0.12, 0.42, 0.44], [0.26, 0.35, 0.30])
            }
        }
        // Both orchard pickups share the transform-fork suspension prefab.
        T.position(-20.0,park_ground_height(-20.0,-4.5,4),-4.5).rotation(0,Math.pi/2.0,0) {
            name="ship_orchard_pickup_north"
            pickup_truck(null,null,null)
        }
        T.position(-20.0,park_ground_height(-20.0,4.5,4),4.5).rotation(0,Math.pi/2.0,0) {
            name="ship_orchard_pickup_south"
            pickup_truck(null,null,null)
        }
        T.position(-30.2, 2.1, 0.0).rotation(0.0, -Math.pi / 2.0, 0.0) {
            ship_label([-0.8, 0.0, 0.0], "CHERRY GROVE", 0.19)
        }
    }
}
