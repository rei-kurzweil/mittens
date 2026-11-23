import { f32, vec3, vec4 } from "types"

// Two peers publish only while they own the pickup. A mount or control change
// claims it; simultaneous claims favor the host. The resting pose stays owned
// by the last driver so a late peer receives it along with the hydraulic state.
export fn pickup_replication(options) {
    let kind = {
        revision: f32, owner: f32, occupied: f32, powered: f32,
        translation: vec3, rotation: vec4, hydraulics: vec4
    }
    let state = { revision = 0 owner = 1 occupied = false elapsed = 0.0 outgoing = null generation = 0 }
    fn owns() { return state.owner == options.rank }
    fn claim() {
        if !state.outgoing { return true }
        if !owns() {
            if state.occupied { return false }
            state.owner = options.rank
            state.revision = state.revision + 1
            options.follow_peer(false, false)
        }
        return true
    }
    fn send() {
        if !owns() || !state.outgoing || !state.outgoing.ready() { return }
        let pose = options.target.world_trs()
        let occupied = 0.0
        let powered = 0.0
        if state.occupied { occupied = 1.0 }
        if options.powered() { powered = 1.0 }
        state.outgoing.write({
            revision = state.revision owner = state.owner occupied = occupied powered = powered
            translation = pose.translation rotation = pose.rotation_quat_xyzw hydraulics = options.hydraulics()
        })
    }
    fn mount_changed(occupied) {
        let was_owner = owns()
        if occupied && !claim() { options.follow_peer(true, true) return }
        if !owns() { return }
        if state.occupied != occupied {
            state.occupied = occupied
            if was_owner { state.revision = state.revision + 1 }
        }
        send()
    }
    fn connect(group, address) {
        state.generation = state.generation + 1
        let generation = state.generation
        let outgoing = options.socket.publish(options.name).to(address).type(kind)
        let incoming = options.socket.subscribe(options.name).from(address).type(kind)
        group.attach(outgoing)
        group.attach(incoming)
        state.outgoing = outgoing
        if options.mounted() { mount_changed(true) }
        if !owns() { options.follow_peer(true, state.occupied) }
        on(incoming, "UdpSample", fn(event) {
            if generation != state.generation { return }
            let value = event.value
            if value.owner != 3 - options.rank || value.revision < state.revision { return }
            if value.revision == state.revision && value.owner > state.owner { return }
            let already_following = !owns()
            let occupied = value.occupied > 0.5
            if !already_following || occupied != state.occupied {
                options.follow_peer(true, occupied)
            }
            state.revision = value.revision
            state.owner = value.owner
            state.occupied = occupied
            options.target.set_translation(value.translation, "world")
            options.target.set_rotation_quat(value.rotation, "world")
            options.apply_controls(value.powered > 0.5, value.hydraulics)
        })
    }
    fn retire_channels() {
        state.generation = state.generation + 1
        state.outgoing = null
    }
    fn disconnect() {
        retire_channels()
        if !owns() {
            state.revision = state.revision + 1
            state.owner = options.rank
            state.occupied = false
            options.follow_peer(false, false)
        }
    }
    let pump = on_global("FrameTick", fn(event) {
        if !state.outgoing { return }
        state.elapsed = state.elapsed + event.dt_sec
        if state.elapsed >= 0.05 { state.elapsed = 0.0 send() }
    })
    fn dispose() {
        retire_channels()
        pump.remove()
    }
    return { claim = claim mount_changed = mount_changed connect = connect disconnect = disconnect
        owns_motion = fn() { return !state.outgoing || owns() } dispose = dispose }
}
