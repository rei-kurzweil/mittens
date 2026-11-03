import { ship_washroom_sign } from "../ship_washroom_sign.mms"
import { ship_store_accordion } from "../ship_store_accordion.mms"
import { ship_food_counter } from "./ship_food_counter.mms"
import { ship_solid, ship_label, ship_deck, ship_door_wall } from "../ship_parts.mms"
import { cafeteria_chair } from "../cafeteria_chair.mms"
import { ship_mall_coffeeshop } from "./ship_mall_coffeeshop.mms"
import { ship_stairs } from "../ship_stairs.mms"
import { ship_path_markers, ship_light_pool } from "../ship_lighting.mms"
import { graphics_settings_panel } from "../panels/graphics_settings_panel.mms"
import { ship_window } from "../ship_window.mms"
import { hot_dog_stand } from "../hot_dog_stand.mms"

// Local X=-11..11, Z=0..12. Upper deck Y=5.2, ceiling Y=8.5.
// Three restaurants along the far wall. Park exit: X=11, Z=9, Y=5.2.

export fn ship_mall_cafeteria() {
    return T {
        name = "ship_mall_cafeteria"
        ship_deck([0.0, 6.0], 22.0, 12.0, 8.5, false)
        ship_door_wall(22.0)
        T.position(-10.84, 2.95, 4.8).rotation(0.0, Math.pi/2.0, 0.0).scale(0.65, 0.65, 0.65) {
            graphics_settings_panel({ root_name = "ship_cafeteria_graphics_settings" title = "Ship graphics" })
        }
        // Three large clerestory windows overlook the bridge/hall roof toward
        // space. Real apertures keep opaque walls out of the glass openings.
        ship_solid([0.0, 4.6, 0.0], [22.0, 2.8, 0.16], [0.40, 0.46, 0.46])
        ship_solid([0.0, 8.2, 0.0], [22.0, 0.6, 0.16], [0.40, 0.46, 0.46])
        for side in [-1.0, 1.0] {
            ship_solid([side*10.5, 6.95, 0.0], [1.0, 1.9, 0.16], [0.40, 0.46, 0.46])
            ship_solid([side*3.7, 6.95, 0.0], [1.6, 1.9, 0.16], [0.40, 0.46, 0.46])
        }
        for index in range(3) {
            T.position(-7.4+index*7.4, 6.95, 0.0) {
                ship_window("ship_cafeteria_space_window_" + index, 5.8, 1.9)
            }
        }
        ship_solid([0.0, 5.9, 12.0], [22.0, 5.2, 0.16], [0.40, 0.46, 0.46])
        // West wall is closed; the vehicle biodome is beyond Cherry Grove.
        // An eye-level side window gives the dining floor its own view too.
        ship_solid([-11.0, 0.40, 6.0], [0.16, 0.8, 12.0], [0.40, 0.46, 0.46])
        ship_solid([-11.0, 3.05, 6.0], [0.16, 0.5, 12.0], [0.40, 0.46, 0.46])
        ship_solid([-11.0, 1.8, 2.5], [0.16, 2.0, 5.0], [0.40, 0.46, 0.46])
        ship_solid([-11.0, 1.8, 10.0], [0.16, 2.0, 4.0], [0.40, 0.46, 0.46])
        T.position(-11.0, 1.8, 6.5).rotation(0.0, Math.pi/2.0, 0.0) {
            ship_window("ship_cafeteria_side_space_window", 3.0, 2.0)
        }
        // Upper west door leads to the washroom corridors.
        ship_solid([-11,4.25,6],[0.16,1.9,12],[0.40,0.46,0.46])
        ship_solid([-11,6.85,4],[0.16,3.3,8],[0.40,0.46,0.46])
        ship_solid([-11,6.85,11],[0.16,3.3,2],[0.40,0.46,0.46])
        ship_solid([-11,8.05,9],[0.16,0.9,2],[0.40,0.46,0.46])
        // East: Cherry Grove upstairs at Z=8..10; ground floor stays closed.
        ship_solid([11,2.6,6],[0.16,5.2,12],[0.40,0.46,0.46])
        ship_solid([11.0,6.85,4.0], [0.16,3.3,8.0], [0.40,0.46,0.46])
        ship_solid([11.0,6.85,11.0], [0.16,3.3,2.0], [0.40,0.46,0.46])
        ship_solid([11.0,8.05,9.0], [0.16,0.9,2.0], [0.40,0.46,0.46])


        // The stair sign points upstairs; the landing sign points west in world space.
        T.position(8.0,2.2,0.9).rotation(0,Math.pi,0) {
            ship_washroom_sign("washrooms","up","WASHROOMS / UPSTAIRS")
        }
        T.position(5.8,7.2,9.95).rotation(0,Math.pi,0) {
            ship_washroom_sign("washrooms","right","WASHROOMS / OTHER SIDE")
        }
        T.position(-10.85,7.6,9.0).rotation(0,Math.pi/2,0) {
            ship_washroom_sign("washrooms","up","WASHROOMS")
        }
        T.position(8.0, 0.0, 1.2) { ship_stairs(1.8, 5.2, 7.0, 28) }
        // Landing connects to the park beyond the east wall. Top stays Y=5.2.
        ship_solid([1.0, 5.10, 9.0], [24.0, 0.20, 1.6], [0.30, 0.36, 0.34])
        ship_solid([0.0, 5.75, 9.84], [22.0, 1.10, 0.08], [0.25, 0.34, 0.31])
        ship_solid([-1.97, 5.75, 8.16], [18.06, 1.10, 0.08], [0.25, 0.34, 0.31])
        for z in [8.35, 9.65] {
            ship_path_markers([-10.3, 5.23, z], [1.0, 0.0], 12, 1.8, [0.16, 0.40, 0.57])
        }
        ship_path_markers([-10.85, 0.14, 1.0], [0.0, 1.0], 6, 1.8, [0.22, 0.58, 0.68])
        ship_path_markers([6.85, 0.035, 1.0], [0.0, 1.0], 4, 1.8, [0.22, 0.58, 0.68])
        for x in [-4.3, 4.3] {
            for z in [3.5, 7.5] {
                ship_light_pool([x, 2.4, z], [1.0, 0.86, 0.68], 2.0, 2.8)
            }
        }
        // Near guard stops at the stair mouth, leaving the ascent open.
        ship_solid([9.97, 5.75, 8.16], [2.06, 1.10, 0.08], [0.25, 0.34, 0.31])
        T.position(7.4, 6.7, 9.95).rotation(0.0, Math.pi, 0.0) {
            ship_label([-1.45, 0.0, 0.0], "CHERRY GROVE > VEHICLE BIODOME / SHOPS", 0.11)
        }
        T.position(8.0, 1.9, 1.1).rotation(0.0, 3.1415927, 0.0) {
            ship_label([-0.65, 0.0, 0.0], "PARKS / SHOPS UPSTAIRS", 0.12)
        }
        T.position(-5,0,12) { ship_store_accordion("ship_coffee_store","ORBIT COFFEE",ship_mall_coffeeshop,6,5,null) }
        T.position(0.5,0,12) { ship_store_accordion("ship_noodle_store","SOLAR NOODLES",fn() {
            return ship_food_counter("SOLAR NOODLES",[0.43,0.24,0.16])
        },5,3,null) }
        T.position(5.5,0,12) { ship_store_accordion("ship_grill_store","HYDROPONIC GRILL",fn() {
            return ship_food_counter("HYDROPONIC GRILL",[0.23,0.37,0.25])
        },5,3,null) }
        // A fourth vendor is an open kiosk against the west wall, in front of
        // the existing storefronts. Keep the central aisle and stair clear.
        T.position(-7.3,0,10.1) { ship_store_accordion("ship_hot_dog_store","ORBIT HOT DOGS",hot_dog_stand,null,null,[-1,2.85,-0.5]) }
        // Tables leave the central walking route clear.
        for x in [-4.3, 4.3] {
            for z in [3.5, 7.5] {
                ship_solid([x, 0.75, z], [2.0, 0.10, 1.0], [0.68, 0.64, 0.46])
                ship_solid([x, 0.35, z], [0.18, 0.70, 0.55], [0.34, 0.41, 0.38])
                for side in [-1.0, 1.0] {
                    T.position(x - 0.55, 0.0, z + side * 0.95).rotation(0.0, (1.0 - side) * 1.5707963, 0.0) { cafeteria_chair() }
                    T.position(x + 0.55, 0.0, z + side * 0.95).rotation(0.0, (1.0 - side) * 1.5707963, 0.0) { cafeteria_chair() }
                }
            }
        }
    }
}
