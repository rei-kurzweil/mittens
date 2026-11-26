import { energy_shield } from "./energy_sheild.mms"

// Energy shields on short stands for a local player to pick up and wear:
// grip the pulsing device and let go, and it mounts on the player's
// shield_mount and turns on. One at a time; letting go of another sends it
// back to its stand. Give the player's health owner the worn one with
// health.wear_shields(pickups.worn) (practice_health: shots then drain the
// shield first, and a rocket blast's shield share). Call put_back() when the
// wearer goes down, to return the worn shield to its stand.
// player: a rooftop player built with shield_mount = true.
// pointers: selectors for the local pointers that can pick one up.
let RAISED = 1.05

export fn shield_pickups(prefix, positions, player, pointers) {
    let state = { worn = null }
    let items = []
    let root = T { name = prefix + "_shield_pickups" }
    for index in range(len(positions)) {
        let p = positions[index]
        let shield = energy_shield(prefix + "_shield_" + index, 2.0)
        let stand = T.position(p[0], p[1], p[2]) {
            name = prefix + "_shield_stand_" + index
            Raycastable.enabled()
            T.position(0.0, 0.45, 0.0).scale(0.22, 0.9, 0.22) {
                R.cube() { C.rgba(0.30, 0.31, 0.35, 1.0) }
                Zone.cube([0.5, 0.5, 0.5]) { Collidable.static() {} }
            }
        }
        stand.attach(shield)
        shield.update_transform([0.0, RAISED, 0.0])
        root.attach(stand)
        items.push({ shield = shield stand = stand })
    }

    fn rest(item) {
        item.stand.attach(item.shield)
        item.shield.update_transform([0.0, RAISED, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
    }
    fn wear(item) {
        if state.worn {
            if state.worn != item { rest(item) }
            return
        }
        player.shield_mount.attach(item.shield)
        item.shield.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
        item.shield.set_worn(true, false)
        state.worn = item
    }
    fn put_back() {
        let item = state.worn
        if !item { return }
        state.worn = null
        item.shield.set_worn(false, false)
        rest(item)
    }
    for selector in pointers {
        let pointer = query(selector)
        if pointer {
            on(pointer, "PointerReleased", fn(event) {
                for item in items {
                    if event.target == item.shield { wear(item) }
                }
            })
        }
    }
    return {
        root = root put_back = put_back
        worn = fn() { if state.worn { return state.worn.shield } return null }
    }
}
