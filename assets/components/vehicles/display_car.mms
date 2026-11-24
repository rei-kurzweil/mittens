// Reusable display-car vehicle fixture.
//
// The car's GLTF and its spatial mounting affordances belong together. The
// entry zone overlaps the lower front of the imported car while extending
// beyond local -Z for an approaching rider. Local -Z is this asset's semantic
// front.
//
// The car is also a carry deck: its cockpit floor and walls, roof, wing tops
// and rear deck collide only inside the car's CollisionContext, so outside it
// the rounded hull spheres keep the car sliding as before. A player whose
// feet get onto or into the car (above the ground, within the board zone)
// rides it, walking on those surfaces while it drives, until they leave the
// stay zone. Leaving the seat stands you on the cockpit floor.

import { ship_vehicle_motion } from "./ship_vehicle_motion.mms"
import { display_car_sliding_colliders } from "./display_car_sliding_colliders.mms"
import { display_car_controls } from "./display_car_controls.mms"
import { display_car_anime_material } from "../materials/display_car_anime.mms"
import { hold_rider_walking } from "./seat_rider_walking.mms"

// A solid box in the car's deck context (car space).
fn deck_box(position, size) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
    }
}

export fn display_car(root_name, position, yaw, mount_anchor_name, extra_children) {
    let entry_zone_frame = T.position(0.0, 1.0, -1.4) {
        name = "car_entry_zone_frame"
    }
    let entry_zone = Zone.cube([4.6, 1.4, 1.4]).at(entry_zone_frame).role("vehicle_entry") {
        name = "car_entry_zone"
    }
    let mount_anchor = T.position(0.0, 4.5, -1.0) {
        name = mount_anchor_name
    }
    // Leaving the seat stands you on the cockpit floor behind it (eye height
    // above the floor at 2.6), where the deck picks you up.
    let dismount_anchor = T.position(0.0, 4.2, 1.2) {
        name = "car_dismount"
    }
    let mountable = Mountable
        .entry_zone(entry_zone)
        .mount_anchor(mount_anchor)
        .dismount_anchor(dismount_anchor)
        .on_grip() {}
    hold_rider_walking(mountable)

    // Board above the ground (so standing beside a moving car never drags
    // you along); stay aboard within a larger box around the whole car.
    let board_frame = T.position(0.0, 3.1, -0.3) { name = "car_board_frame" }
    let board = Zone.cube([5.0, 2.75, 3.6]).at(board_frame) { name = "car_board_zone" }
    let stay_frame = T.position(0.0, 2.8, -0.3) { name = "car_stay_frame" }
    let stay = Zone.cube([5.8, 3.6, 4.4]).at(stay_frame) { name = "car_stay_zone" }

    let model = GLTF.new("assets/models/car.glb") {
        display_car_anime_material("near")
    }
    on(model, "GLTFInitialized", fn(event) {
        // Only these closed split primitives pass solid import. Damage remains
        // per wing and per vehicle; the rounded movement hull stays independent.
        for name in ["vehicle_body_wing_left", "vehicle_body_wing_right"] {
            for part_mesh in event.gltf.query_all("[name='" + name + "'] > renderable") {
                let damage = Destroyable.threshold(3.0) { name="car_wing_damage" }
                part_mesh.attach(damage)
            }
        }
    })
    // The deck's surfaces, measured from car.glb (body y 2.34 to 5.05 and
    // x +-1.58, wings x 1.58 to 4.4 with tops at 5.05, roof top at 5.57):
    // the cockpit floor (2.6) with cabin side and back walls, the roof, the
    // wing tops and the deck behind the roof. Imported meshes only collide
    // when they are Destroyable, so these are explicit boxes.
    let deck = CollisionContext {
        name = root_name + "_deck_context"
        T {
            name = "car_model"
            model
        }
        deck_box([0.0, 2.5, 0.55], [3.1, 0.2, 4.3])
        for side in [-1.0, 1.0] {
            deck_box([side * 1.62, 3.83, 0.57], [0.1, 2.45, 4.37])
            deck_box([side * 2.99, 4.95, 0.77], [2.82, 0.2, 3.95])
        }
        deck_box([0.0, 3.83, 2.75], [3.2, 2.45, 0.1])
        deck_box([0.0, 5.47, 0.13], [3.74, 0.2, 3.47])
        deck_box([0.0, 4.95, 2.31], [3.17, 0.2, 0.88])
    }
    let deck_mountable = Mountable.carry().entry_zone(stay).auto_mount_zone(board).context(deck) {
        name = root_name + "_deck"
    }

    let body = T.position(position[0], position[1], position[2]).rotation(0.0, yaw, 0.0) {
        name = root_name
        mountable
        entry_zone_frame
        entry_zone
        mount_anchor
        dismount_anchor
        board_frame
        board
        stay_frame
        stay
        deck_mountable
        deck

        for child in extra_children { child }
    }
    body.attach(display_car_sliding_colliders(body))
    // The driver's seat; its controls ignore riders boarding the deck.
    body.seat = mountable
    let motion = ship_vehicle_motion(body, root_name + "_velocity")
    let gravity = Gravity { name = root_name + "_gravity" }
    motion.attach(gravity)
    return motion
}


// Plain fixtures retain physics without registering driving controls.
export fn display_car_desktop(root_name, position, yaw, mount_anchor_name) {
    let motion = display_car(root_name, position, yaw, mount_anchor_name, [])
    let body = motion.query("#" + root_name)
    body.attach(display_car_controls(body, motion, "desktop", null))
    return motion
}

export fn display_car_xr(root_name, position, yaw, mount_anchor_name, xr_gamepad) {
    let motion = display_car(root_name, position, yaw, mount_anchor_name, [])
    let body = motion.query("#" + root_name)
    body.attach(display_car_controls(body, motion, "xr", xr_gamepad))
    return motion
}
