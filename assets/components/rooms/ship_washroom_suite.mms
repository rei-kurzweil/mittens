import { ship_washroom_passage } from "./ship_washroom_passage.mms"
import { ship_womens_washroom } from "./ship_womens_washroom.mms"
import { washroom_mirror_zones } from "./ship_washroom_mirror_zones.mms"
import { ship_tile_solid } from "../ship_tile_solid.mms"
import { ship_environment_material } from "../materials/ship_anime_materials.mms"

// Fits a 14-by-12-m shell. Enter from +X at z=3.2. The outer clerestory
// is on the -Z facade at x=-5, -3, -1 and y=4.35.
export fn ship_washroom_suite(player_probe) {
    return T {
        name = "ship_washroom_suite"
        ship_environment_material()
        T.position(17.0, -5.185, -11.8) {
            ship_washroom_passage()
            T.position(-17.0,5.2,12.0).rotation(0.0,Math.pi,0.0) { ship_womens_washroom() }
        }
        washroom_mirror_zones({
            mirror = "#ship_womens_washroom_mirror"
            player_probe = player_probe
            entrance = [-1.0, 1.265, 1.0]
            exit = [7.4, 1.25, 3.2]
            exit_yaw = Math.pi / 2.0
        })
        for side in [-1.0, 1.0] {
            ship_tile_solid([6.5, 1.4, 3.2 + side * 1.0], [1.0, 2.8, 0.16], null)
        }
        // A shallow threshold bridges the 23 cm between the street and floor.
        T.position(7.65, -0.115, 3.2).rotation(0.0, -Math.pi / 2.0, 0.0) {
            T.rotation(Math.atan2(-0.23, 1.30), 0.0, 0.0) {
                ship_tile_solid([0.0, -0.08, 0.0], [2.04, 0.16, Math.sqrt(1.30*1.30 + 0.23*0.23)], null)
            }
        }
    }
}
