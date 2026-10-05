// position/size describe a non-solid world-local trigger volume. Destination
// is the mover zone's world center, so accumulated Input offsets do not move
// the respawn point. Visuals: "spikes", "none", or a factory returning a fresh mesh component.
// Decorations are deterministic jittered patches; they have no Collidable.
export fn teleport_pit(pit_name, position, size, destination, visual) {
    let sensor = Zone.cube([size[0] * 0.5, size[1] * 0.5, size[2] * 0.5]).enable_events() {
        name = pit_name + "_sensor"
    }
    on(sensor, "ZoneEntered", fn(event) {
        let offset = event.movement_target_offset
        event.movement_target.teleport_world([
            destination[0] + offset[0],
            destination[1] + offset[1],
            destination[2] + offset[2],
        ])
    })
    return T.position(position[0], position[1], position[2]) {
        name = pit_name
        sensor
        if visual != "none" {
            for patch in range(24) {
                // Fixed seed arithmetic keeps layouts repeatable across loads.
                let x = ((patch * 37 + 11) % 101) / 100.0 - 0.5
                let z = ((patch * 61 + 23) % 103) / 102.0 - 0.5
                T.position(x * size[0], -size[1] * 0.5 - 0.6, z * size[2]).scale(0.65, 0.65, 1.2).rotation(-1.5707963, 0.0, 0.0) {
                    if visual == "spikes" {
                        R.cone() { C.rgba(0.40, 0.16, 0.12, 1.0) }
                    } else {
                        visual()
                    }
                }
            }
        }
    }
}
