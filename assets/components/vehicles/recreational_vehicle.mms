import { ship_vehicle_motion } from "./ship_vehicle_motion.mms"
import { vehicle_glass, vehicle_window_wall } from "./vehicle_glass.mms"
import { ship_solid, ship_lamp } from "../ship_parts.mms"
import { ship_vehicle_wheel } from "./ship_vehicle_parts.mms"
import { ship_stairs } from "../ship_stairs.mms"

// Destroyable retained body panels; wheels remain visual trim.
// White cab-over motorhome, nose toward -Z. slide_extension is metres (0..1.2).
// The left lounge section expands horizontally; choose its extension per instance.
// Side door, collidable interior floor and beds are accessible for blockout review.
export fn recreational_vehicle(slide_extension) {
    if slide_extension == null { slide_extension = 0.0 }
    let extension = Math.clamp(slide_extension, 0.0, 1.2)
    let beige = [0.90, 0.90, 0.86]
    let body = T {
        name = "recreational_vehicle"
        Destroyable.threshold(3.0)
        CombineMesh.keep_transforms() {
            Material(MaterialDefinition.anime())
            ship_solid([0.0, 0.61, 0.1], [2.8, 0.16, 6.4], [0.31, 0.29, 0.22])
            ship_solid([0.0, 3.02, 0.55], [2.8, 0.14, 5.5], beige)
            vehicle_window_wall([0,1.82,3.3],[2.8,2.4,0.12],[0,2.12,3.3],[1.6,0.65],beige,true)
            // Right side wraps around the real 0.9 m entry opening at Z=0.7.
            vehicle_window_wall([1.4,1.82,-0.925],[0.10,2.4,2.35],[1.4,2.12,-1.1],[1.2,0.65],beige,false)
            vehicle_window_wall([1.4,1.82,2.225],[0.10,2.4,2.15],[1.4,2.12,2.25],[1.35,0.65],beige,false)
            ship_solid([1.4, 2.86, 0.7], [0.10, 0.32, 0.9], beige)

            // Slide-out surrounds an opening in the left wall; its floor bridges
            // the original room and moving outer face at every authored extension.
            ship_solid([-1.4, 1.82, -1.45], [0.10, 2.4, 1.3], beige)
            ship_solid([-1.4, 1.82, 2.7], [0.10, 2.4, 1.2], beige)
            ship_solid([-1.4-extension/2.0, 0.67, 0.65], [0.20+extension, 0.12, 2.9], beige)
            ship_solid([-1.4-extension/2.0, 2.86, 0.65], [0.20+extension, 0.32, 2.9], beige)
            vehicle_window_wall([-1.45-extension,1.82,0.65],[0.10,2.4,2.9],[-1.45-extension,2.12,0.65],[1.9,0.65],beige,false)
            for z in [-0.8, 2.1] {
                ship_solid([-1.4-extension/2.0, 1.82, z], [0.20+extension, 2.4, 0.10], beige)
            }
            ship_solid([-0.95-extension, 1.0, 0.65], [0.72, 0.60, 2.2], [0.39, 0.35, 0.22])
            ship_solid([0.0, 0.94, 2.5], [1.65, 0.50, 1.4], [0.47, 0.46, 0.32])
            ship_solid([0.0, 1.22, 2.5], [1.65, 0.08, 1.4], [0.83, 0.77, 0.59])
            // Cab and projecting sleeping bunk above its windshield.
            ship_solid([0.0, 1.12, -2.65], [2.5, 0.95, 1.5], beige)
            ship_solid([0.0, 2.51, -2.95], [2.8, 0.12, 2.5], beige)
            ship_solid([0.0, 3.02, -2.95], [2.8, 0.14, 2.5], beige)
            for side in [-1.0, 1.0] {
                vehicle_window_wall([side*1.36,2.77,-2.95],[0.08,0.45,2.5],[side*1.36,2.78,-3.0],[1.1,0.29],beige,false)
            }
            ship_solid([0.0, 2.61, -3.0], [2.3, 0.08, 1.8], [0.81, 0.75, 0.56])
        }
        vehicle_glass([1.46,2.12,2.25],1.35,0.65,Math.pi/2)
        vehicle_glass([1.46,2.12,-1.1],1.2,0.65,Math.pi/2)
        vehicle_glass([0,2.12,3.37],1.6,0.65,0)
        vehicle_glass([-1.51-extension,2.12,0.65],1.9,0.65,-Math.pi/2)
        vehicle_glass([0,1.95,-3.42],2.2,0.65,Math.pi)
        vehicle_glass([0,2.78,-4.22],2.7,0.40,Math.pi)
        for side in [-1,1] { vehicle_glass([side*1.41,2.78,-3],1.1,0.29,side*Math.pi/2) }
        for x in [-1.35, 1.35] {
            for z in [-2.25, 2.1] { ship_vehicle_wheel(x, z, 0.42) }
        }
        for x in [-0.85, 0.85] {
            ship_lamp([x, 1.0, -3.43], [0.30, 0.15, 0.04], [0.80, 0.70, 0.42])
        }
        // Shallow entry stair flight reaches the cabin deck without a jump.
        T.position(2.65, 0.0, 0.7).rotation(0.0, -Math.pi / 2.0, 0.0) {
            ship_stairs(0.78, 0.69, 1.2, 5)
        }
    }
    return ship_vehicle_motion(body,"recreational_vehicle_velocity")
}
