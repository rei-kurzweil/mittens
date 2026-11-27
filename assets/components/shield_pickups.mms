import { energy_shield } from "./energy_sheild.mms"
import { npc_action_window } from "./network/npc_replication.mms"

// Energy shields on short stands for a local player to pick up and wear:
// grip the pulsing device and let go, and it mounts on the player's
// shield_mount and turns on. One at a time; letting go of another sends it
// back to its stand. Give the player's health owner the worn one with
// health.wear_shields(pickups.worn) (practice_health: shots then drain the
// shield first, and a rocket blast's shield share). Call put_back() when the
// wearer goes down, to return the worn shield to its stand.
// player: a rooftop player built with shield_mount = true.
// pointers: selectors for the local pointers that can pick one up.
// network (optional): { socket rank } to share them with one peer. Picking a
// shield up or putting it back sends an event (each once, in order); the
// peer then shows that shield worn on your avatar, or back on its stand, and
// can't take it while you wear it. If both take the same one at once, the
// lower rank (the host) keeps it. Call connect(group, address, remote) with
// the peer's rooftop player (built with shield_mount = true) and
// disconnect() as the peer joins and leaves.
let RAISED = 1.05
// Shield events on the wire; the argument is the shield's index.
let TAKEN = 1.0
let RETURNED = 2.0

export fn shield_pickups(prefix, positions, player, pointers, network) {
    // worn: the item on the local player. remote: the peer's player while
    // connected; an item's remote flag marks it worn by them.
    let state = { worn = null remote = null }
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
        items.push({ shield = shield stand = stand index = index remote = false })
    }

    let events = { window = null }
    fn send(kind, item) {
        if events.window { events.window.push(kind, item.index, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]) }
    }
    fn rest(item) {
        item.stand.attach(item.shield)
        item.shield.update_transform([0.0, RAISED, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
    }
    fn mount_on(item, mount) {
        mount.attach(item.shield)
        item.shield.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
        item.shield.set_worn(true, false)
    }
    // Back on its stand, worn by nobody.
    fn release(item) {
        item.remote = false
        item.shield.set_worn(false, false)
        rest(item)
    }
    fn wear(item) {
        if state.worn || item.remote {
            if state.worn != item { rest(item) }
            return
        }
        mount_on(item, player.shield_mount)
        state.worn = item
        send(TAKEN, item)
    }
    fn put_back() {
        let item = state.worn
        if !item { return }
        state.worn = null
        release(item)
        send(RETURNED, item)
    }
    fn item_at(index) {
        for item in items { if item.index == index { return item } }
        return null
    }
    fn apply(sequence, kind, index, position, rotation) {
        let item = item_at(Math.floor(index))
        if !item || !state.remote { return }
        if kind == TAKEN {
            if state.worn == item {
                // Both took it at once: the lower rank (the host) keeps it.
                if network.rank < 2 { return }
                state.worn = null
            }
            item.remote = true
            mount_on(item, state.remote.shield_mount)
        }
        if kind == RETURNED && item.remote { release(item) }
    }
    if network {
        events.window = npc_action_window({ socket = network.socket name = prefix + ".shields" apply = apply })
    }
    fn connect(group, address, remote) {
        if !events.window { return }
        state.remote = remote
        events.window.connect(group, address)
        // The peer learns what this player already wears.
        if state.worn { send(TAKEN, state.worn) }
    }
    fn disconnect() {
        if !events.window { return }
        events.window.disconnect()
        for item in items { if item.remote { release(item) } }
        state.remote = null
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
        root = root put_back = put_back connect = connect disconnect = disconnect
        worn = fn() { if state.worn { return state.worn.shield } return null }
    }
}
