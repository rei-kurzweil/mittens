import { ship_tile_solid } from "../ship_tile_solid.mms"
import { ship_washroom_sign } from "../ship_washroom_sign.mms"
import { ship_light_pool, ship_path_markers } from "../ship_lighting.mms"

// Upper landing, west of the cafeteria: opposite Cherry Grove. Women turn
// around the corner to the south; the men's corridor currently ends at a cap.
export fn ship_washroom_passage() {
    return T.position(0,5.2,0) {
        name="ship_washroom_passage"
        for y in [-0.1,3.3] {
            ship_tile_solid([-17,y,15],[12,0.2,2],null)
            ship_tile_solid([-18,y,13],[2,0.2,2],null)
        }
        ship_tile_solid([-17,1.6,16],[12,3.2,0.16],null)
        ship_tile_solid([-21,1.6,14],[4,3.2,0.16],null)
        ship_tile_solid([-14,1.6,14],[6,3.2,0.16],null)
        for x in [-19,-17] { ship_tile_solid([x,1.6,13],[0.16,3.2,2],null) }
        T {name="ship_mens_washroom_hallway_stub"
            ship_tile_solid([-23,1.6,15],[0.16,3.2,2],null)
            T.position(-22.88,2.0,15).rotation(0,Math.pi/2,0) {
                ship_washroom_sign("men","none","MEN / NOT OPEN YET")
            }
        }
        T.position(-17.0,2.1,15).rotation(0,Math.pi/2,0) {
            ship_washroom_sign("women","right","WOMEN / AROUND CORNER")
        }
        ship_path_markers([-11.5,0.06,15.85],[-1,0],8,1.4,[0.24,0.58,0.67])
        ship_light_pool([-16,2.8,15],[0.76,0.87,0.90],4.0,7.0)
        ship_light_pool([-18,2.8,12.5],[0.76,0.87,0.90],3.0,4.0)
    }
}
