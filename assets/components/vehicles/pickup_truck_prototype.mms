import { ship_vehicle_motion } from "./ship_vehicle_motion.mms"
import { vehicle_glass } from "./vehicle_glass.mms"
import { pickup_cab_interior } from "./pickup_cab_interior.mms"
import { pickup_driving } from "./pickup_driving.mms"
import { ship_solid, ship_lamp, ship_label } from "../ship_parts.mms"
import { pickup_suspension } from "./pickup_suspension.mms"
import { pickup_anime_material } from "../materials/pickup_anime.mms"

// pickup_truck is the chassis transform driven by Velocity and used for mounting.
// Destroyable retained body panels; suspension/wheels remain separate branches.
// Parked pickup, front toward -Z. Open cargo bed and a two-seat cab.
fn make_pickup(color, cargo) {
    if color == null { color = [0.59, 0.52, 0.42] }
    let finish = pickup_anime_material()
    let suspensions = [
        pickup_suspension("fl",-0.96,-1.65,0.40,finish),
        pickup_suspension("fr", 0.96,-1.65,0.40,finish),
        pickup_suspension("rl",-0.96, 1.55,0.40,finish),
        pickup_suspension("rr", 0.96, 1.55,0.40,finish),
    ]
    let truck = T {
        name = "pickup_truck"
        Destroyable.threshold(3.0)
        CombineMesh.keep_transforms() {
            name = "pickup_painted_body"
            finish
            ship_solid([0.0, 0.65, 0.0], [2.0, 0.30, 4.8], color)
            ship_solid([0.0,1.0,-1.72],[2.0,0.45,1.30],color)
            // Hollow cabin: no solid cube through the seats or footwell.
            ship_solid([0,2.30,-0.40],[2.0,0.12,1.8],color)
            ship_solid([0,1.08,0.40],[1.9,0.56,0.10],color)
            ship_solid([0,2.16,0.40],[1.9,0.16,0.10],color)
            for side in [-1,1] {
                ship_solid([side*0.94,1.73,-1.22],[0.09,1.02,0.09],color)
                ship_solid([side*0.94,1.70,0.40],[0.09,1.05,0.09],color)
                // Open doorways allow inspecting and entering the cabin.
                ship_solid([side*0.94,0.87,-0.40],[0.10,0.14,1.55],color)
            }
            pickup_cab_interior()
            for side in [-1.0, 1.0] {
                ship_solid([side * 0.95, 1.10, 1.35], [0.10, 0.65, 2.1], color)
                ship_solid([side * 0.72, 1.12, -2.42], [0.38, 0.20, 0.04], [0.72, 0.67, 0.46])
            }
            ship_solid([0.0, 1.10, 2.35], [2.0, 0.65, 0.10], color)
            ship_solid([0.0, 0.68, -2.48], [2.10, 0.16, 0.12], [0.51, 0.53, 0.48])
        }
        vehicle_glass([0,1.75,-1.23],1.80,0.85,Math.pi)
        vehicle_glass([0,1.75,0.455],1.80,0.80,0)
        for side in [-1.0, 1.0] {
            ship_lamp([side * 0.85, 1.0, 2.42], [0.15, 0.24, 0.04], [0.60, 0.08, 0.04])
        }
        for suspension in suspensions { suspension }
        T.position(-0.53,1.235,-0.797) { ship_label([0,0,0],"00000.0 km",0.025) }
        T.position(-0.77,1.66,-0.80) { ship_label([0,0,0],"RPM     SPEED    FUEL",0.026) }
        // Bed floor top is 0.8 m; cargo is attached to the vehicle, not the room.
        T.position(0.0, 0.82, 1.35) {
            name = "pickup_bed_cargo_mount"
            if cargo != null { cargo }
        }
    }
    truck.set_suspension_compression = fn(corner,ratio) {
        if corner == null {
            for suspension in suspensions { suspension.set_compression(ratio) }
        } else {
            let suspension = truck.query("#suspension_mount_"+corner)
            if suspension { suspension.set_compression(ratio) }
        }
    }
    let motion = ship_vehicle_motion(truck,"pickup_velocity")
    truck.attach(pickup_driving(truck,motion))
    return motion
}

export fn pickup_truck(color, cargo) {
    return make_pickup(color,cargo)
}
