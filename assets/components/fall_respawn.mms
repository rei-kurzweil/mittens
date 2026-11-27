// A wide, non-solid catch volume under a play area. Anything with a
// Collidable that falls into it (the player, an NPC) stops and reappears at
// a spot spread over the area, `drop` metres above `floor_y`. Holes in the
// area are fine; a respawn into one just falls back here.
// area: { min_x max_x min_z max_z }. depth: how far below floor_y the volume
// starts (it is 20 m tall and 200 m wider than the area on each side).
fn noise(value) {
    let raw = Math.sin(value * 12.9898 + 78.233) * 43758.5453
    return raw - Math.floor(raw)
}

export fn fall_respawn(name, area, floor_y, depth) {
    let drop = 1.0
    let state = { respawns = 0 }
    let width = area.max_x - area.min_x + 400.0
    let length = area.max_z - area.min_z + 400.0
    let sensor = Zone.cube([width * 0.5, 10.0, length * 0.5]).enable_events() {
        name = name + "_sensor"
    }
    on(sensor, "ZoneEntered", fn(event) {
        let target = event.movement_target
        state.respawns = state.respawns + 1
        let step = state.respawns * 2.0
        let x = area.min_x + (area.max_x - area.min_x) * noise(step)
        let z = area.min_z + (area.max_z - area.min_z) * noise(step + 1.0)
        let velocity = target.query("../Velocity")
        if velocity { velocity.reset() }
        // The target moves in its parent's space; assume an upright,
        // unscaled parent and shift by the parent's world origin.
        let world = target.world_trs().translation
        let local = target.trs().translation
        let parent = [world[0] - local[0], world[1] - local[1], world[2] - local[2]]
        target.update_transform([x - parent[0], floor_y + drop - parent[1], z - parent[2]])
    })
    let cx = (area.min_x + area.max_x) * 0.5
    let cz = (area.min_z + area.max_z) * 0.5
    return T.position(cx, floor_y - depth - 10.0, cz) {
        name = name
        sensor
    }
}
