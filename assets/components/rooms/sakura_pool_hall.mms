import { ceramic_parts, building_box, combined_ceramic, building_collisions, destroyable_section } from "../buildings/pool_building.mms"
import { indoor_pool } from "./indoor-pool.mms"

// A 14 x 12 m pool building whose hall is sunk 3.1 m below the street, so the
// 8.9 m hall fits under a two-storey roof. The street side is +Z. The closed
// shell, stair hallway and excavation walls always exist; the hall itself
// (tiles, slide, lights and the reflective water with its material) is built
// when the player comes within about 3 m of the building, however they
// arrive, and removed once they are more than about 6 m away. Pass this footprint to sakura_twilight_street
// as a ground hole: the hall's floor is below the terrain.
//
// Hallway: the door at x = -5.5 opens onto a landing, stairs run down along
// +X beside the hall, then the lower corridor turns -Z into a vestibule whose
// doorway faces the hall's entrance. The turn keeps the doorway out of sight
// from the stairs.
let WIDTH = 14.0
let DEPTH = 12.0
let ROOF = 6.4
let ROOF_UNDER = ROOF - 0.22
let SHELL_BOTTOM = -0.23
let LANDING_Y = -0.22
let RISE = 0.16
let TREAD = 0.28
let STEPS = 17
let DECK_Y = LANDING_Y - (STEPS + 1) * RISE
let DOOR = { center = -5.5 width = 2.0 height = 2.4 }
// Hall placement: its entrance wall's inner face is at x = 3.88, the doorway
// centred on z = -2, and its length runs toward -X.
let HALL = { x = 3.88 z = -2.0 width = 8.2 length = 10.6 basin_width = 3.5 basin_length = 8.0 }
let CORRIDOR = { z0 = 3.16 z1 = DEPTH / 2.0 - 0.12 wall = 3.0 }
let VESTIBULE = { x0 = 4.04 x1 = WIDTH / 2.0 - 0.12 z0 = -3.82 }
let STAIRS_X = -4.0

fn box_between(data, x0, x1, y0, y1, z0, z1, trim) {
    building_box(data, [(x0 + x1) / 2.0, (y0 + y1) / 2.0, (z0 + z1) / 2.0], [x1 - x0, y1 - y0, z1 - z0], trim)
}

// One horizontal band around all four facades; the street side can leave
// a doorway gap.
fn shell_band(data, y0, y1, trim, door) {
    let hx = WIDTH / 2.0
    let hz = DEPTH / 2.0
    box_between(data, -hx - 0.12, hx + 0.12, y0, y1, -hz - 0.12, -hz + 0.12, trim)
    for side in [-1.0, 1.0] {
        box_between(data, side * hx - 0.12, side * hx + 0.12, y0, y1, -hz + 0.12, hz - 0.12, trim)
    }
    if door {
        box_between(data, -hx - 0.12, DOOR.center - DOOR.width / 2.0, y0, y1, hz - 0.12, hz + 0.12, trim)
        box_between(data, DOOR.center + DOOR.width / 2.0, hx + 0.12, y0, y1, hz - 0.12, hz + 0.12, trim)
    } else {
        box_between(data, -hx - 0.12, hx + 0.12, y0, y1, hz - 0.12, hz + 0.12, trim)
    }
}

// Closed facades (no window bands: the hall is not there to see), a trim line
// per storey, the roof slab and low parapets.
fn shell_parts() {
    let data = ceramic_parts(0.0)
    shell_band(data, SHELL_BOTTOM, DOOR.height, false, true)
    shell_band(data, DOOR.height, 2.98, false, false)
    shell_band(data, 2.98, 3.2, true, false)
    shell_band(data, 3.2, 6.0, false, false)
    shell_band(data, 6.0, ROOF, true, false)
    box_between(data, -WIDTH / 2.0, WIDTH / 2.0, ROOF_UNDER, ROOF, -DEPTH / 2.0, DEPTH / 2.0, false)
    for side in [-1.0, 1.0] {
        building_box(data, [side * WIDTH / 2.0, ROOF + 0.30, 0.0], [0.22, 0.60, DEPTH], true)
        building_box(data, [0.0, ROOF + 0.30, side * DEPTH / 2.0], [WIDTH, 0.60, 0.22], true)
    }
    return data
}

// Excavation walls, the landing, stairs, corridor and vestibule.
fn hallway_parts() {
    let data = ceramic_parts(0.0)
    let hx = WIDTH / 2.0
    let hz = DEPTH / 2.0
    let base = DECK_Y - 0.2
    box_between(data, -hx - 0.12, hx + 0.12, -4.7, SHELL_BOTTOM, -hz - 0.12, -hz + 0.12, false)
    box_between(data, -hx - 0.12, hx + 0.12, -4.7, SHELL_BOTTOM, hz - 0.12, hz + 0.12, false)
    for side in [-1.0, 1.0] {
        box_between(data, side * hx - 0.12, side * hx + 0.12, -4.7, SHELL_BOTTOM, -hz + 0.12, hz - 0.12, false)
    }
    // The landing reaches through the doorway to meet the street grass.
    box_between(data, -hx + 0.12, STAIRS_X, LANDING_Y - 0.2, LANDING_Y, CORRIDOR.z0, hz + 0.12, false)
    for step in range(STEPS) {
        let x = STAIRS_X + step * TREAD
        box_between(data, x, x + TREAD, base, LANDING_Y - (step + 1) * RISE, CORRIDOR.z0, CORRIDOR.z1, false)
    }
    let stairs_end = STAIRS_X + STEPS * TREAD
    box_between(data, stairs_end, VESTIBULE.x1, base, DECK_Y, CORRIDOR.z0, CORRIDOR.z1, false)
    // The vestibule floor runs under the hall's entrance wall to its deck.
    box_between(data, HALL.x, VESTIBULE.x1, base, DECK_Y, VESTIBULE.z0, CORRIDOR.z0, false)
    // Corridor wall beside the hall, rising to the roof.
    box_between(data, -hx + 0.12, VESTIBULE.x0 + 0.16, base, ROOF_UNDER, CORRIDOR.wall, CORRIDOR.z0, false)
    // Vestibule: the west wall has a doorway matching the hall's own.
    let door0 = HALL.z - 1.25
    let door1 = HALL.z + 1.25
    let wx0 = VESTIBULE.x0
    let wx1 = VESTIBULE.x0 + 0.16
    box_between(data, wx0, wx1, base, ROOF_UNDER, VESTIBULE.z0 - 0.16, door0, false)
    box_between(data, wx0, wx1, base, ROOF_UNDER, door1, CORRIDOR.wall, false)
    box_between(data, wx0, wx1, DECK_Y + 2.4, ROOF_UNDER, door0, door1, false)
    box_between(data, wx1, VESTIBULE.x1, base, ROOF_UNDER, VESTIBULE.z0 - 0.16, VESTIBULE.z0, false)
    return data
}

fn light(position) {
    return T.position(position[0], position[1], position[2]) {
        PL.color(0.80, 0.90, 0.94).intensity(4.0).distance(7.0)
    }
}

fn player_zone(zone, signal, player_probe, action) {
    on(zone, signal, fn(event) {
        if event.other_zone == player_probe { action() }
    })
}

// player_probe: the local player's sensor zone (as for the washroom suite).
export fn sakura_pool_hall(label, origin, tiles, trim_color, player_probe) {
    let white = [0.94, 0.96, 0.97]
    let hallway = hallway_parts()
    let mount = T { name = label + "_interior_mount" }
    let state = { hall = null stop_water = null }

    fn load() {
        if state.hall { return }
        let pool = indoor_pool([0.0, 0.0, 0.0], HALL.width, HALL.length, HALL.basin_width, HALL.basin_length, true)
        state.stop_water = pool.stop_water
        let hall = T {
            name = label + "_interior"
            T.position(HALL.x, DECK_Y, HALL.z).rotation(0.0, -Math.pi / 2.0, 0.0) { pool }
            light([STAIRS_X + STEPS * TREAD / 2.0, 1.2, (CORRIDOR.z0 + CORRIDOR.z1) / 2.0])
            light([(VESTIBULE.x0 + VESTIBULE.x1) / 2.0, DECK_Y + 2.6, 0.0])
        }
        state.hall = hall
        mount.attach(hall)
    }
    fn unload() {
        if !state.hall { return }
        if state.stop_water { state.stop_water() }
        state.hall.remove_subtree()
        state.hall = null
        state.stop_water = null
    }

    // Streaming is gated on getting close, however the player arrives (door,
    // roof, a hole in a wall): entering a box about 3 m around the building
    // loads the hall; leaving a box about 6 m around it unloads it. The gap
    // between them is a deadband.
    let load_zone = Zone.cube([WIDTH / 2.0 + 3.0, 6.5, DEPTH / 2.0 + 3.0]).enable_events() {
        name = label + "_load_zone"
    }
    let unload_zone = Zone.cube([WIDTH / 2.0 + 6.0, 8.0, DEPTH / 2.0 + 6.0]).enable_events() {
        name = label + "_unload_zone"
    }
    player_zone(load_zone, "ZoneEntered", player_probe, load)
    player_zone(unload_zone, "ZoneExited", player_probe, unload)

    let root = T.position(origin[0], origin[1], origin[2]) {
        name = label
        destroyable_section(label + "_shell", shell_parts(), white, trim_color, tiles)
        T {
            name = label + "_hallway"
            combined_ceramic(label + "_hallway_white", hallway.white, hallway.white_count, white, tiles)
            building_collisions(label + "_hallway", hallway)
        }
        mount
        T.position(0.0, 1.0, 0.0) { load_zone }
        T.position(0.0, 1.0, 0.0) { unload_zone }
    }
    root.load_interior = load
    root.unload_interior = unload
    return root
}

// The footprint as a sakura_twilight_street ground hole.
export fn sakura_pool_hall_hole(origin) {
    return [origin[0] - WIDTH / 2.0, origin[0] + WIDTH / 2.0, origin[2] - DEPTH / 2.0, origin[2] + DEPTH / 2.0]
}
