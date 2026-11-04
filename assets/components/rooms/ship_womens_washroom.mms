import { ship_washroom_sign } from "../ship_washroom_sign.mms"
import { ship_tile_solid as ship_solid } from "../ship_tile_solid.mms"
import { ship_window } from "../ship_window.mms"
import { ship_box, ship_label } from "../ship_parts.mms"
import { ship_light_pool } from "../ship_lighting.mms"
import { toilet } from "../toilet.mms"
import { hand_dryer, paper_towel_dispenser, washroom_sink } from "../washroom_fixtures.mms"

// Local X=0..6, Z=0..6. Entrance at X=1, Z=0, from the turned hallway.
export fn ship_womens_washroom() {
    return T {
        name = "ship_womens_washroom"
        // Five-metre tiled room. The ship places this on the upper west deck.
        ship_solid([3,-0.1,3], [6,0.2,6], null)
        ship_solid([3,5.0,3], [6,0.16,6], null)
        ship_light_pool([3.0, 4.5, 3.0], [0.74, 0.83, 0.73], 5.0, 7.0)
        T.position(1.0,2.45,-0.12).rotation(0,Math.pi,0) {
            ship_washroom_sign("women","none","WOMEN")
        }
        // A 1.8 m opening aligned with the 2 m approach corridor.
        ship_solid([0.05, 2.0, 0.0], [0.1, 4.0, 0.16], [0.52, 0.57, 0.52])
        ship_solid([3.95, 2.0, 0.0], [4.1, 4.0, 0.16], [0.52, 0.57, 0.52])
        ship_solid([1.0, 3.3, 0.0], [1.8, 1.4, 0.16], [0.52, 0.57, 0.52])
        ship_solid([0.0, 2.5, 3.0], [0.16, 5.0, 6.0], [0.52, 0.57, 0.52])
        ship_solid([3.0, 2.0, 6.0], [6.0, 4.0, 0.16], [0.52, 0.57, 0.52])
        // East wall has a real 0.8 x 0.82 m recess beside the towel station.
        // Keep the entire vanity span open behind the mirror. The reflected
        // camera sits east of this wall and must see through to the player.
        // Only an invisible pressure boundary prevents walking into that void.
        T.position(6.0, 2.5, 1.45) {
            name = "ship_washroom_mirror_camera_aperture"
            Zone.cube([0.08, 2.5, 1.45]) { Collidable.static() {} }
        }
        ship_solid([6.0, 2.5, 3.9], [0.16, 5.0, 2.0], [0.52, 0.57, 0.52])
        ship_solid([6.0, 2.5, 5.85], [0.16, 5.0, 0.30], [0.52, 0.57, 0.52])
        ship_solid([6.0, 0.14, 5.3], [0.16, 0.28, 0.8], [0.52, 0.57, 0.52])
        ship_solid([6.0, 3.05, 5.3], [0.16, 3.90, 0.8], [0.52, 0.57, 0.52])
        ship_solid([6.36, 0.70, 5.3], [0.08, 0.84, 0.8], [0.13, 0.19, 0.17])
        for z in [4.88, 5.72] {
            ship_solid([6.18, 0.70, z], [0.36, 0.84, 0.04], [0.30, 0.38, 0.34])
        }
        for y in [0.28, 1.12] {
            ship_solid([6.18, y, 5.3], [0.36, 0.04, 0.88], [0.30, 0.38, 0.34])
        }
        T { name = "ship_washroom_recessed_bin"
            ship_box([6.18, 0.49, 5.3], [0.26, 0.36, 0.64], [0.32, 0.40, 0.35])
            ship_box([6.18, 0.68, 5.3], [0.22, 0.01, 0.58], [0.045, 0.07, 0.055])
        }
        // Small clerestory windows: solid tile jambs, clear views of space.
        for z in [0.0, 6.0] {
            ship_solid([3,4.85,z], [6,0.30,0.16], null)
            for x in [0.2,2.0,4.0,5.8] {
                let width = 0.8
                if x == 0.2 || x == 5.8 { width = 0.4 }
                ship_solid([x,4.35,z], [width,0.70,0.16], null)
            }
            for index in range(3) {
                T.position(1+index*2,4.35,z) {
                    ship_window("ship_washroom_high_window_" + z + "_" + index,1.2,0.70)
                }
            }
        }
        T.position(3.0,2.15,5.89).rotation(0,Math.pi,0) {
            ship_label([-1.5,0,0],"WOMEN'S WASHROOM",0.18)
        }
        // Three west-side stalls face the sinks/dryers across a clear aisle.
        for z in [1.40, 2.90, 4.40, 5.90] {
            ship_solid([1.15, 1.10, z], [2.30, 2.20, 0.06], [0.30, 0.43, 0.39])
        }
        for z in [2.15, 3.65, 5.15] {
            T.position(0.65, 0.0, z).rotation(0.0, -Math.pi/2.0, 0.0) { toilet() }
            for side in [-1.0, 1.0] {
                ship_solid([2.30, 1.10, z + side*0.65], [0.06, 2.20, 0.40], [0.30, 0.43, 0.39])
            }
            // Doors stand partly open into the stalls, leaving the aisle free.
            T.position(2.30, 0.0, z+0.44).rotation(0.0, 0.9, 0.0) {
                ship_solid([0.0, 1.10, -0.42], [0.055, 2.0, 0.84], [0.36, 0.49, 0.43])
                ship_box([0.04, 1.12, -0.72], [0.04, 0.07, 0.06], [0.66, 0.71, 0.65])
            }
        }
        // From the entrance: sinks and wide mirror, dryers, towels, recessed bin.
        ship_solid([5.45, 0.85, 1.55], [0.80, 0.14, 2.50], [0.72, 0.76, 0.67])
        for z in [0.95, 2.15] {
            T.position(5.42, 0.93, z).rotation(0.0, -Math.pi/2.0, 0.0) { washroom_sink() }
        }
        for z in [3.20, 4.05] {
            T.position(5.80, 1.35, z).rotation(0.0, -Math.pi/2.0, 0.0) { hand_dryer() }
        }
        T.position(5.82, 1.45, 4.75).rotation(0.0, -Math.pi/2.0, 0.0) { paper_towel_dispenser() }
        // Horizontal mirror above both sinks; its +Z face turns into the room.
        T.position(5.85, 1.80, 1.55).rotation(0.0, -Math.pi/2.0, 0.0) {
            // Open frame, with no backing panel or rear mirror face.
            for side in [-1.0, 1.0] {
                ship_box([side*1.2375, 0.0, 0.0], [0.075, 0.99, 0.06], [0.38, 0.46, 0.41])
                ship_box([0.0, side*0.46, 0.0], [2.40, 0.07, 0.06], [0.38, 0.46, 0.41])
            }
            T.scale(2.40, 0.85, 1.0) {
                name = "ship_washroom_wide_mirror"
                R.plane() { Mirror.quality(512).enabled(false) { name="ship_womens_washroom_mirror" } Raycastable.enabled() }
            }
        }
    }
}
