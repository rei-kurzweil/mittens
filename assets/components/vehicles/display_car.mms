// Reusable display-car vehicle fixture.
//
// The car's GLTF and its spatial mounting affordances belong together. The
// entry zone overlaps the lower front of the imported car while extending
// beyond local -Z for an approaching rider. Local -Z is this asset's semantic
// front.

import { ship_vehicle_motion } from "./ship_vehicle_motion.mms"
import { display_car_sliding_colliders } from "./display_car_sliding_colliders.mms"
import { display_car_controls } from "./display_car_controls.mms"
import { display_car_anime_material } from "../materials/display_car_anime.mms"

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
    let dismount_anchor = T.position(0.0, 2.4, -4.6) {
        name = "car_dismount"
    }
    let mountable = Mountable
        .entry_zone(entry_zone)
        .mount_anchor(mount_anchor)
        .dismount_anchor(dismount_anchor)
        .on_grip() {}

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

    let body = T.position(position[0], position[1], position[2]).rotation(0.0, yaw, 0.0) {
        name = root_name
        mountable
        entry_zone_frame
        entry_zone
        mount_anchor
        dismount_anchor

        T {
            name = "car_model"
            model
        }

        for child in extra_children { child }
    }
    body.attach(display_car_sliding_colliders(body))
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
