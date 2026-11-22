// Hinged cab doors, like the STA launcher's breech: each imported door node
// goes under a TOP.hinge at its front edge and gets a Grabbable, so a grip
// swings it open. Its panel collider stays on the door mesh and swings with
// it. With a socket, each hinge's local pose is published while this peer
// holds the door and received otherwise.
import { vec3, vec4 } from "types"

let pose_type = { translation: vec3, rotation_quat_xyzw: vec4, scale: vec3 }
let open_limit = 70.0 * Math.pi / 180.0

// In the GLB's `truck` node frame: local X is vertical, the left door sits at
// +Y and the right at -Y, and each door node's origin is on its front edge
// (truck-local z = 0). Rotating about +X swings a +Y door's rear outward.
export fn pickup_doors(vehicle_id) {
    let entries = [
        { name = "door_left" axis = [1.0, 0.0, 0.0] door = null hinge = null holder = null outgoing = null incoming = null },
        { name = "door_right" axis = [-1.0, 0.0, 0.0] door = null hinge = null holder = null outgoing = null incoming = null },
    ]
    fn update_bindings(entry) {
        let local = entry.holder != null
        if entry.outgoing { entry.outgoing.set_binding_enabled(local) }
        if entry.incoming { entry.incoming.set_binding_enabled(!local) }
    }
    fn hang(entry, door, chassis, color) {
        let rest = door.trs()
        let hinge = TOP.hinge(entry.axis).limits(0.0, open_limit) { name = "pickup_" + entry.name + "_hinge" }
        let pivot = T.position(rest.translation[0], rest.translation[1], 0.0) {
            name = "pickup_" + entry.name + "_pivot"
            hinge
        }
        chassis.attach(pivot)
        hinge.attach(door)
        door.trs({
            translation = [0.0, 0.0, rest.translation[2]]
            rotation_quat_xyzw = rest.rotation_quat_xyzw
            scale = rest.scale
        })
        let grab = Grabbable {}
        door.attach(grab)
        // Outside the batch the door keeps its own primitive colour.
        if color != null {
            for part in door.query_all("renderable") { part.set_color([color[0], color[1], color[2], 1.0]) }
        }
        entry.door = door
        entry.hinge = hinge
        on(door, "GrabStart", fn(event) {
            if event.target != door { return }
            entry.holder = event.pointer
            update_bindings(entry)
        })
        on(door, "GrabEnd", fn(event) {
            if event.target != door || event.pointer != entry.holder { return }
            entry.holder = null
            update_bindings(entry)
        })
    }
    // Call from GLTFInitialized before batching: the doors must stay out of
    // the static CombineMesh so they can move.
    fn install(gltf, chassis, color) {
        for entry in entries {
            let door = gltf.query("[name='" + entry.name + "']")
            if door { hang(entry, door, chassis, color) }
        }
    }
    fn connect(socket, group, address) {
        for entry in entries {
            if entry.hinge {
                let channel = vehicle_id + "." + entry.name
                entry.outgoing = socket.publish(channel).to(address).type(pose_type).bind_transform(entry.hinge)
                entry.incoming = socket.subscribe(channel).from(address).type(pose_type).bind_transform(entry.hinge)
                update_bindings(entry)
                group.attach(entry.outgoing)
                group.attach(entry.incoming)
            }
        }
    }
    fn disconnect() {
        for entry in entries {
            entry.outgoing = null
            entry.incoming = null
        }
    }
    return { install = install connect = connect disconnect = disconnect }
}
