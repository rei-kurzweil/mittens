// Two-peer prototype policy, authored entirely in MMS. Latest transition wins;
// simultaneous transitions favor rank 2 (client). This is not host authorization.
//
// The owner streams the pose only while it holds the item. Grab, release and
// drops onto a shared mount are events: one record, repeated briefly so a lost
// packet does not matter. A mounted item's pose is local to its mount, so each
// peer's copy then rides its own copy of that mount with no further traffic.
import { f32, vec3, vec4 } from "types"

let world_mount = 0
// Event records repeat on the 50 ms send cadence: about 200 ms in total.
let event_repeats = 4

// options: { socket, name, target, rank, mounts? } where mounts comes from
// shared_mounts() and is shared by every item and mount in the scene.
// Weapons can also pass stock: { capacity, apply(amount, restocked) }.
export fn shared_grabbable(options) {
    let target = options.target
    // Identity mount: incoming world poses are also local here. Reattaching a
    // losing local hold invalidates native following through existing lifecycle.
    let mount = T { name = options.name + "_mount" target }
    let stock = options["stock"]
    let on_mount = options["on_mount"]
    let home = null
    let kind = { revision: f32, owner: f32, held: f32, mount: f32, translation: vec3, rotation: vec4 }
    let state = { revision = 0 owner = 1 held = false mount = world_mount pointer = null outgoing = null
        elapsed = 0.0 generation = 0 repeats = 0 pump = null ammo = null restocks = 0 }
    if stock {
        home = target.trs()
        state.ammo = stock.capacity
        kind = { revision: f32, owner: f32, held: f32, mount: f32, translation: vec3, rotation: vec4, ammo: f32, restocks: f32 }
    }
    fn own() { return state.owner == options.rank }
    fn sync_pump() {
        let active = own() && state.outgoing != null && (state.held || state.repeats > 0)
        if !active { state.elapsed = 0.0 }
        state.pump.bypass(!active)
    }
    fn mount_frame(id) {
        if id == world_mount || !options.mounts { return null }
        return options.mounts.frame(id)
    }
    fn mark_mount(id) {
        if state.mount == id { return }
        state.mount = id
        if on_mount { on_mount(id) }
    }
    fn apply_pose(value) {
        if value.mount == world_mount {
            target.set_translation(value.translation, "world")
            target.set_rotation_quat(value.rotation, "world")
            return
        }
        let scale = target.trs().scale
        target.trs({ translation = value.translation rotation_quat_xyzw = value.rotation scale = scale })
    }
    // Returns whether a record was written; channels need a moment to declare.
    fn send() {
        if !own() || !state.outgoing || !state.outgoing.ready() { return false }
        let pose = target.world_trs()
        if state.mount != world_mount { pose = target.trs() }
        let held = 0.0
        if state.held { held = 1.0 }
        let sample = { revision = state.revision owner = state.owner held = held mount = state.mount
            translation = pose.translation rotation = pose.rotation_quat_xyzw }
        if stock { sample.ammo = state.ammo sample.restocks = state.restocks }
        state.outgoing.write(sample)
        return true
    }
    fn announce() {
        state.repeats = event_repeats
        if send() { state.repeats = state.repeats - 1 }
        sync_pump()
    }
    fn set_ammo(amount) {
        if !stock || !own() || state.ammo == amount { return }
        state.ammo = amount
        announce()
    }
    fn place(id, pose) {
        if !own() { return }
        let frame = mount
        if id != world_mount { frame = mount_frame(id) }
        if !frame { return }
        frame.attach(target)
        if id == world_mount { target.world_trs(pose) }
        else { target.trs(pose) }
        state.revision = state.revision + 1
        state.pointer = null
        state.held = false
        mark_mount(id)
        state.repeats = event_repeats
        sync_pump()
    }
    fn restock() {
        if !stock { return }
        state.restocks = state.restocks + 1
        state.revision = state.revision + 1
        state.owner = options.rank
        state.pointer = null
        state.held = false
        mark_mount(world_mount)
        state.ammo = stock.capacity
        mount.attach(target)
        target.trs(home)
        stock.apply(state.ammo, true)
        // The next pump reads the pose after the attach and transform intents.
        state.repeats = event_repeats
        sync_pump()
    }
    fn grabbed(event) {
        if event.target != target { return }
        state.revision = state.revision + 1
        state.owner = options.rank
        state.held = true
        mark_mount(world_mount)
        state.pointer = event.pointer
        announce()
    }
    fn released(event) {
        if event.target != target || event.pointer != state.pointer { return }
        state.pointer = null
        if !own() { return }
        state.revision = state.revision + 1
        state.held = false
        mark_mount(world_mount)
        announce()
    }
    // Called by attach_drops after a release lands in a shared mount zone.
    // It follows the release record, so the higher revision wins remotely.
    target.shared_mount_changed = fn(id) {
        if !own() { return }
        state.revision = state.revision + 1
        state.held = false
        mark_mount(id)
        announce()
    }
    fn bind_pointer(pointer) {
        on(pointer, "PointerGrabbed", grabbed)
        on(pointer, "PointerReleased", released)
    }
    fn connect(group, address) {
        state.generation = state.generation + 1
        let generation = state.generation
        let outgoing = options.socket.publish(options.name).to(address).type(kind)
        let incoming = options.socket.subscribe(options.name).from(address).type(kind)
        group.attach(outgoing)
        group.attach(incoming)
        state.outgoing = outgoing
        on(incoming, "UdpSample", fn(event) {
            if generation != state.generation { return }
            let value = event.value
            // Only the other participant may publish its own ownership record.
            if value.owner != 3 - options.rank { return }
            let restocked = stock && value.restocks > state.restocks
            if stock && value.restocks < state.restocks { return }
            if !restocked {
                if value.revision < state.revision { return }
                if value.revision == state.revision && value.owner < state.owner { return }
            }
            let frame = mount_frame(value.mount)
            // An unknown mount leaves the revision unconsumed for a later repeat.
            if value.mount != world_mount && !frame { return }
            if own() && state.held {
                // Stop local following before applying the winning remote pose.
                // PointerReleased from native cleanup must not claim it again.
                mount.attach(target)
            }
            if value.mount != world_mount {
                frame.attach(target)
            } else if (state.mount != world_mount || restocked) && !(own() && state.held) {
                mount.attach(target)
            }
            state.revision = value.revision
            state.owner = value.owner
            state.held = value.held > 0.5
            mark_mount(value.mount)
            state.repeats = 0
            apply_pose(value)
            if stock && (restocked || value.ammo != state.ammo) {
                state.restocks = value.restocks
                state.ammo = value.ammo
                stock.apply(state.ammo, restocked)
            }
            sync_pump()
        })
        // A newly connected peer learns the current state once.
        announce()
    }
    fn disconnect() {
        state.generation = state.generation + 1
        state.outgoing = null
        state.repeats = 0
        if !own() {
            state.revision = state.revision + 1
            state.owner = options.rank
            state.held = false
            state.pointer = null
        }
        sync_pump()
    }
    state.pump = on_global("FrameTick", fn(event) {
        state.elapsed = state.elapsed + event.dt_sec
        if state.elapsed < 0.05 { return }
        state.elapsed = 0.0
        let sent = send()
        if sent && state.repeats > 0 { state.repeats = state.repeats - 1 }
        if !state.held && state.repeats == 0 { sync_pump() }
    })
    state.pump.bypass(true)
    return { root = mount target = target state = state bind_pointer = bind_pointer connect = connect disconnect = disconnect
        set_ammo = set_ammo restock = restock place = place }
}
