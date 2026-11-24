import { energy_shield } from "../energy_sheild.mms"
import { shared_grabbable } from "./shared_grabbable.mms"

export fn arena_shields(socket, rank, mounts, local, remote, positions, can_equip) {
    let local_slot = 100 + rank
    let remote_slot = 103 - rank
    mounts.register(local_slot, local.shield_mount)
    mounts.register(remote_slot, remote.shield_mount)
    let state = { items = [] count = len(positions) }
    let wear_pose = { translation = [0.0, 0.0, 0.0] rotation_quat_xyzw = [0.0, 0.0, 0.0, 1.0] scale = [1.0, 1.0, 1.0] }

    fn equipped(slot) {
        for item in state.items {
            if item.state.mount == slot { return item.target }
        }
        return null
    }
    fn make(id, position) {
        let shield = energy_shield("arena_shield_" + id, 2.0)
        shield.update_transform(position)
        let grab = shared_grabbable({
            socket = socket rank = rank name = "arena.shield." + id target = shield mounts = mounts
            on_mount = fn(slot) { shield.set_worn(slot == local_slot || slot == remote_slot, slot == remote_slot) }
        })
        shield.wear_key = fn() { return id + state.count * grab.state.revision }
        return grab
    }
    // Two to four pickups, at the given positions.
    if state.count == 2 { state.items = [make(1, positions[0]), make(2, positions[1])] }
    if state.count == 3 {
        state.items = [make(1, positions[0]), make(2, positions[1]), make(3, positions[2])]
    }
    if state.count >= 4 {
        state.count = 4
        state.items = [make(1, positions[0]), make(2, positions[1]), make(3, positions[2]), make(4, positions[3])]
    }
    let root = T {
        name = "arena_shields"
        for item in state.items { item.root }
    }
    fn bind_pointer(pointer) {
        for item in state.items { item.bind_pointer(pointer) }
        on(pointer, "PointerReleased", fn(event) {
            if equipped(local_slot) || (can_equip && !can_equip(null)) { return }
            for item in state.items {
                if event.target == item.target && item.state.owner == rank { item.place(local_slot, wear_pose) }
            }
        })
    }
    fn drop_at(item, player) {
        let position = player.root.world_trs().translation
        item.place(0, { translation = [position[0], position[1] + 0.24, position[2]]
            rotation_quat_xyzw = wear_pose.rotation_quat_xyzw scale = wear_pose.scale })
    }
    fn drop_local() {
        for item in state.items {
            if item.state.mount == local_slot { drop_at(item, local) }
        }
    }
    fn connect(group, address) {
        for item in state.items { item.connect(group, address) }
    }
    fn disconnect() {
        for item in state.items {
            let was_remote = item.state.mount == remote_slot
            item.disconnect()
            if was_remote { drop_at(item, remote) }
        }
    }
    return { root = root bind_pointer = bind_pointer connect = connect disconnect = disconnect drop_local = drop_local
        local_equipped = fn() { return equipped(local_slot) }
        remote_equipped = fn() { return equipped(remote_slot) }
        local_mount = local.shield_mount remote_mount = remote.shield_mount }
}
