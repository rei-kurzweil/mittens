import { mount_equipment } from "./avatar_equipment.mms"

// NPC Mitten's leg rockets: loose STA rounds in the thigh equipment slots,
// collected from STA ammo cabinets. A round on each thigh enables them, as
// for players (rocket_flight_equipment.mms): faster running, and flight.

let SIDES = ["left", "right"]

fn rotate(q, v) {
    let ix = q[3] * v[0] + q[1] * v[2] - q[2] * v[1]
    let iy = q[3] * v[1] + q[2] * v[0] - q[0] * v[2]
    let iz = q[3] * v[2] + q[0] * v[1] - q[1] * v[0]
    let iw = -q[0] * v[0] - q[1] * v[1] - q[2] * v[2]
    return [
        ix * q[3] - iw * q[0] - iy * q[2] + iz * q[1],
        iy * q[3] - iw * q[1] - iz * q[0] + ix * q[2],
        iz * q[3] - iw * q[2] - ix * q[1] + iy * q[0],
    ]
}

// cabinets: [{ root, label, approach }]: an sta_ammo_cabinet, its label and
// the floor point to walk to. A round is available only while it still sits
// in its shelf slot, so rounds a player moved, or that are already on a
// thigh or in a launcher, are skipped. paths has plan(here, goal).
export fn mitten_rocket_supply(cabinets, paths) {
    // sta_ammo_cabinet's layout: 4 shelves of 5 rounds.
    fn slot_point(cabinet, row, column) {
        let pose = cabinet.root.world_trs()
        let r = rotate(pose.rotation_quat_xyzw, [(column - 2) * 0.15, 0.18 + 0.43 * row + 0.179, 0.10])
        return [pose.translation[0] + r[0], pose.translation[1] + r[1], pose.translation[2] + r[2]]
    }
    fn shelved(cabinet) {
        let rounds = []
        for row in range(4) {
            for column in range(5) {
                let round = cabinet.root.query("[name='" + cabinet.label + "_round_" + row + "_" + column + "']")
                if round && round.query("#sta_rocket_ammo_zone") {
                    let p = round.world_trs().translation
                    let s = slot_point(cabinet, row, column)
                    let d = [p[0] - s[0], p[1] - s[1], p[2] - s[2]]
                    if d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < 0.0025 { rounds.push(round) }
                }
            }
        }
        return rounds
    }
    // The cabinet with rounds that is shortest to walk to, with its route.
    fn nearest(here) {
        let best = null
        for cabinet in cabinets {
            if len(shelved(cabinet)) > 0 {
                let route = paths.plan(here, cabinet.approach)
                if route && (!best || route.length < best.route.length) {
                    best = { cabinet = cabinet route = route }
                }
            }
        }
        return best
    }
    fn near(cabinet, here) {
        let dx = cabinet.approach[0] - here[0]
        let dz = cabinet.approach[2] - here[2]
        return dx * dx + dz * dz < 0.6 * 0.6
    }
    // Up to `count` rounds from this cabinet, off their shelves.
    fn take(cabinet, count) {
        let rounds = shelved(cabinet)
        let taken = []
        for round in rounds {
            if len(taken) < count { taken.push(round) }
        }
        return taken
    }
    return { nearest = nearest near = near take = take }
}

// model: the NPC's GLTF root holding the equipment slots.
export fn mitten_leg_rockets(model) {
    let state = { mode = null }
    fn round_on(side) {
        for item in model.query_all("#equipment_" + side + "_thigh > transform") {
            // Unclaimed rounds keep their ammo probe; claimed ones are spent.
            if item.claim_sta_round && item.query("#sta_rocket_ammo_zone") { return item }
        }
        return null
    }
    fn count() {
        let total = 0
        for side in SIDES { if round_on(side) { total = total + 1 } }
        return total
    }
    fn enabled() { return round_on("left") != null && round_on("right") != null }
    // "thrust", "hover" or null (off). Only both rounds together burn.
    fn set_mode(mode) {
        if !enabled() { mode = null }
        if mode == state.mode { return }
        state.mode = mode
        for side in SIDES {
            let round = round_on(side)
            if round {
                if mode { round.activate(mode) } else { round.deactivate() }
            }
        }
    }
    // Fills empty thigh slots from rounds; returns how many were mounted.
    fn mount(rounds) {
        let mounted = 0
        for side in SIDES {
            if !round_on(side) && mounted < len(rounds) {
                if mount_equipment(model, "equipment_" + side + "_thigh", rounds[mounted]) { mounted = mounted + 1 }
            }
        }
        // Reapply the current mode so a new round burns with the other.
        let mode = state.mode
        state.mode = "refresh"
        set_mode(mode)
        return mounted
    }
    return { count = count enabled = enabled mount = mount set_mode = set_mode
        missing = fn() { return 2 - count() } }
}
