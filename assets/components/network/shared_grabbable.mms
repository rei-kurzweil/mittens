// Two-peer prototype policy, authored entirely in MMS. Latest transition wins;
// simultaneous transitions favor rank 2 (client). This is not host authorization.
import { f32, vec3, vec4 } from "types"

export fn shared_grabbable(options) {
    let target = options.target
    // Identity mount: incoming world poses are also local here. Reattaching a
    // losing local hold invalidates native following through existing lifecycle.
    let mount = T { name = options.name + "_mount" target }
    let kind = { revision: f32, owner: f32, held: f32, translation: vec3, rotation: vec4 }
    let state = { revision = 0 owner = 1 held = false pointer = null outgoing = null elapsed = 0.0 generation = 0 }
    fn own() { return state.owner == options.rank }
    fn apply_pose(value) {
        target.set_translation(value.translation, "world")
        target.set_rotation_quat(value.rotation, "world")
    }
    fn send() {
        if !own() || !state.outgoing || !state.outgoing.ready() { return }
        let pose = target.world_trs()
        let held = 0.0
        if state.held { held = 1.0 }
        state.outgoing.write({ revision = state.revision owner = state.owner held = held
            translation = pose.translation rotation = pose.rotation_quat_xyzw })
    }
    fn grabbed(event) {
        if event.target != target { return }
        state.revision = state.revision + 1
        state.owner = options.rank
        state.held = true
        state.pointer = event.pointer
        send()
    }
    fn released(event) {
        if event.target != target || event.pointer != state.pointer { return }
        state.pointer = null
        if !own() { return }
        state.revision = state.revision + 1
        state.held = false
        send()
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
            if value.revision < state.revision { return }
            if value.revision == state.revision && value.owner < state.owner { return }
            if own() && state.held {
                // Stop local following before applying the winning remote pose.
                // PointerReleased from native cleanup must not claim it again.
                mount.attach(target)
            }
            state.revision = value.revision
            state.owner = value.owner
            state.held = value.held > 0.5
            apply_pose(value)
        })
        send()
    }
    fn disconnect() {
        state.generation = state.generation + 1
        state.outgoing = null
        if !own() {
            state.revision = state.revision + 1
            state.owner = options.rank
            state.held = false
            state.pointer = null
        }
    }
    on_global("FrameTick", fn(event) {
        state.elapsed = state.elapsed + event.dt_sec
        if state.elapsed >= 0.05 { state.elapsed = 0.0 send() }
    })
    return { root = mount target = target state = state bind_pointer = bind_pointer connect = connect disconnect = disconnect }
}
