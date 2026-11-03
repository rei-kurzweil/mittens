import { ship_tile_solid } from "../ship_tile_solid.mms"
import { ship_label } from "../ship_parts.mms"
import { ship_light_pool, ship_path_markers } from "../ship_lighting.mms"

// Cafeteria east door is [11,0,10] in ship space. A tiled right-angle
// passage reaches the washroom entrance at [16,0,12]. No bridge-hall link.
export fn ship_washroom_passage() {
    return T {
        name = "ship_washroom_passage"
        for y in [-0.1, 3.3] {
            ship_tile_solid([14.0,y,10.0], [6.0,0.2,2.0], null)
            ship_tile_solid([16.0,y,11.5], [2.0,0.2,1.0], null)
        }
        ship_tile_solid([14.0,1.6,9.0], [6.0,3.2,0.16], null)
        ship_tile_solid([13.0,1.6,11.0], [4.0,3.2,0.16], null)
        ship_tile_solid([17.0,1.6,10.5], [0.16,3.2,3.0], null)
        ship_tile_solid([15.0,1.6,11.5], [0.16,3.2,1.0], null)
        T.position(16.88,2.15,9.7).rotation(0.0,-Math.pi/2.0,0.0) {
            ship_label([-0.55,0,0], "WOMEN'S\nWASHROOM ->", 0.14)
        }
        ship_path_markers([11.5,0.06,9.12], [1,0], 4, 1.4, [0.24,0.58,0.67])
        ship_light_pool([14.0,2.8,10.0], [0.76,0.87,0.90], 3.5, 5.0)
    }
}
