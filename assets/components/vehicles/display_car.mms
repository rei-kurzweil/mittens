// Reusable display-car vehicle fixture.
//
// The car's GLTF and its spatial mounting affordances belong together. The
// entry zone overlaps the lower front of the imported car while extending
// beyond local -Z for an approaching rider. Local -Z is this asset's semantic
// front.

import { bisket_anime_shading } from "../materials/bisket_anime_shading.mms"

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

    return T.position(position[0], position[1], position[2]).rotation(0.0, yaw, 0.0) {
        name = root_name
        mountable
        entry_zone_frame
        entry_zone
        mount_anchor
        dismount_anchor

        T {
            name = "car_model"
            GLTF.new("assets/models/car.glb") {
                bisket_anime_shading()
            }
        }

        for child in extra_children { child }
    }
}
