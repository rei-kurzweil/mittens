import { loose_rocket_exhaust } from "./rocket_exhaust.mms"

let ammo_zone_name = "sta_rocket_ammo_zone"

fn sta_rocket_model() {
    return T {
        name = "sta_rocket_model"
        T.position(0.191356, -1.507589, -0.111893) {
            GLTF.new("assets/models/STA_rocket_bullet.glb") {}
        }
    }
}

export fn sta_rocket_body(name) {
    return T { name = name sta_rocket_model() }
}

export fn sta_rocket_ammo(name) {
    let state = { claimed = false exhaust = null }
    let pickup = Grabbable {}
    let pickup_target = Raycastable.enabled()
    let round = T {
        name = name
        pickup
        pickup_target
        sta_rocket_model()
    }
    let probe = T.position(0.0, 0.0, -0.11) {
        Zone.sphere(0.035) {
            name = ammo_zone_name
            Collidable.sensor().movement_target(round)
        }
    }
    round.attach(probe)
    round.align_to_equipment_slot = true
    round.activate = fn(mode) {
        if state.claimed { return }
        if !state.exhaust { state.exhaust = loose_rocket_exhaust(round) }
        state.exhaust.activate(mode)
    }
    round.deactivate = fn() {
        if state.exhaust { state.exhaust.deactivate() }
    }
    round.claim_sta_round = fn() {
        if state.claimed { return false }
        round.deactivate()
        state.claimed = true
        pickup.remove_subtree()
        pickup_target.remove_subtree()
        probe.remove_subtree()
        return true
    }
    return round
}

export fn claim_sta_rocket_ammo(event) {
    let round = event.movement_target
    if round.query("[name='" + ammo_zone_name + "']") != event.other_zone { return null }
    if !round.claim_sta_round() { return null }
    return round
}
