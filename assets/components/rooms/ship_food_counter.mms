import { ship_solid, ship_label } from "../ship_parts.mms"
import { ship_light_pool } from "../ship_lighting.mms"

export fn ship_food_counter(title,color) {
    return T {
        ship_solid([0,3.3,1.5],[5,0.16,3],[0.35,0.39,0.39])
        ship_light_pool([0,2.6,0.7],[1,0.86,0.65],6,4)
        ship_solid([0,1.6,3],[5,3.2,0.16],color)
        for side in [-1,1] { ship_solid([side*2.5,1.6,1.5],[0.16,3.2,3],color) }
        ship_solid([-0.4,0.5,0.8],[3.6,1,0.75],color)
        ship_solid([-0.4,1.04,0.8],[3.8,0.08,0.9],[0.66,0.66,0.53])
        T.position(0,2.5,0).rotation(0,Math.pi,0) { ship_label([-1.4,0,0],title,0.21) }
    }
}
