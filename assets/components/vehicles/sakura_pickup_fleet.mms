import { f32 } from "types"
import { pickup_truck } from "./pickup_truck.mms"

export fn sakura_pickup_fleet(socket, rank, mounts, ground_y, single_pickup) {
    let state = { group = null address = null outgoing = null repeats = 0 sync_elapsed = 0.0 }
    let kind = { gold: f32, blue: f32 }
    let lower = ground_y - 50.0
    let upper = ground_y + 100.0
    fn parking_slot(id, name, x, z, color) {
        let frame = T.position(x, ground_y, z).rotation(0, -Math.pi / 2.0, 0) { name = name }
        return { id = id color = color frame = frame truck = null motion = null channels = null generation = 0 stage = null }
    }
    let gold = parking_slot(1, "sakura_twilight_pickup", -5.0, 4.5, [1.0, 0.86, 0.34])
    let blue = null
    let slots = [gold]
    if !single_pickup {
        blue = parking_slot(2, "sakura_twilight_ground_pickup", 5.0, -6.0, [1.0, 0.86, 0.34])
        slots = [gold, blue]
    }

    fn connect_truck(slot) {
        if !state.group || slot.stage { return }
        let channels = T {}
        state.group.attach(channels)
        slot.channels = channels
        slot.motion.connect(channels, state.address)
    }
    fn spawn(slot) {
        let name = "arena.pickup." + slot.id
        if slot.generation > 0 { name = name + ".life." + slot.generation }
        slot.motion = pickup_truck(slot.color, null, true, {
            socket = socket vehicle_id = name rank = rank mounts = mounts bed_mount = slot.id acceleration_drive = true
        })
        slot.frame.attach(slot.motion)
        slot.truck = slot.motion.query("#pickup_truck")
        slot.stage = null
        connect_truck(slot)
    }
    for slot in slots { spawn(slot) }

    fn queue_respawn(slot, generation) {
        if generation <= slot.generation { return }
        slot.generation = generation
        if !slot.stage { slot.stage = "retire" }
    }
    fn respawn() {
        for slot in slots {
            if !slot.stage { queue_respawn(slot, slot.generation + 1) }
        }
        state.repeats = 4
    }
    fn check_bounds(slot) {
        if slot.stage || !slot.motion.owns_motion() { return }
        let y = slot.truck.world_trs().translation[1]
        if y <= lower || y >= upper {
            queue_respawn(slot, slot.generation + 1)
            state.repeats = 4
        }
    }
    fn catch_truck(event) {
        for slot in slots {
            if event.movement_target == slot.truck { check_bounds(slot) }
        }
    }
    let low_box = Zone.cube([220, 100, 120]).enable_events() { name = "sakura_pickup_lower_bound" }
    let high_box = Zone.cube([220, 100, 120]).enable_events() { name = "sakura_pickup_upper_bound" }
    on(low_box, "ZoneEntered", catch_truck)
    on(high_box, "ZoneEntered", catch_truck)

    fn reset_step(slot) {
        if slot.stage == "retire" {
            slot.motion.dispose_replication()
            if slot.channels { slot.channels.remove_subtree() slot.channels = null }
            slot.truck.set_powered(false)
            slot.motion.reset()
            slot.truck.trs({ translation = [0, 0, 0] rotation_quat_xyzw = [0, 0, 0, 1] scale = [1, 1, 1] })
            slot.stage = "dismount"
        } else if slot.stage == "dismount" {
            // Keep the old anchors available for native rider cleanup at ground level.
            for seat in slot.motion.query_all("#pickup_truck > mountable") { seat.remove_subtree() }
            for item in slot.truck.query_all("#pickup_bed_cargo_zone > transform") {
                let pose = item.world_trs()
                item.detach()
                item.world_trs(pose)
                if item.shared_mount_changed { item.shared_mount_changed(0) }
            }
            slot.stage = "replace"
        } else if slot.stage == "replace" && !slot.truck.query("#local_root") {
            slot.motion.remove_subtree()
            spawn(slot)
        }
    }
    fn tick(dt) {
        for slot in slots {
            // Also catches a fast mover that skips the box or goes beyond its footprint.
            check_bounds(slot)
            if slot.stage { reset_step(slot) }
        }
        if !state.outgoing || !state.outgoing.ready() { return }
        state.sync_elapsed = state.sync_elapsed + dt
        // Periodically repair a missed burst after either truck has respawned.
        let blue_generation = 0
        if blue { blue_generation = blue.generation }
        let active = gold.generation > 0 || blue_generation > 0
        if state.repeats > 0 || (active && state.sync_elapsed >= 2.0) {
            state.outgoing.write({ gold = gold.generation blue = blue_generation })
            state.sync_elapsed = 0.0
            if state.repeats > 0 { state.repeats = state.repeats - 1 }
        }
    }
    fn disconnect() {
        for slot in slots {
            if slot.channels {
                slot.motion.disconnect()
                slot.channels.remove_subtree()
                slot.channels = null
            }
        }
        state.outgoing = null
        state.group = null
        state.address = null
    }
    fn connect(group, address) {
        state.group = group
        state.address = address
        let incoming = socket.subscribe("arena.pickup.respawn").from(address).type(kind)
        state.outgoing = socket.publish("arena.pickup.respawn").to(address).type(kind)
        group.attach(incoming)
        group.attach(state.outgoing)
        for slot in slots { connect_truck(slot) }
        state.repeats = 4
        state.sync_elapsed = 0.0
        on(incoming, "UdpSample", fn(sample) {
            if state.group != group { return }
            queue_respawn(gold, sample.value.gold)
            if blue { queue_respawn(blue, sample.value.blue) }
        })
    }
    let fleet = T {
        name = "sakura_pickup_fleet"
        for slot in slots { slot.frame }
        T.position(44, lower - 100.0, -13) { low_box }
        T.position(44, upper + 100.0, -13) { high_box }
    }
    return { root = fleet tick = tick connect = connect disconnect = disconnect respawn = respawn }
}
