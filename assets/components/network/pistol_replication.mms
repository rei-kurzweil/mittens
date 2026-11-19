import { f32, vec3, vec4 } from "types"

// Each cadence slot has a latest-value channel. Briefly resend a shot to cover
// packet loss. Sequence checks prevent a resend from playing it twice.
export fn pistol_replication(options) {
    let kind = { sequence: f32, mode: f32, translation: vec3, rotation: vec4, scale: vec3 }
    let link = { generation = 0 elapsed = 0.0 }
    fn make_slot(index) {
        return { index = index outgoing = null sent = 0 seen = 0 sample = null remaining = 0.0 }
    }
    let slots = [make_slot(0), make_slot(1), make_slot(2)]
    fn send(slot) {
        if slot.sample && slot.outgoing && slot.outgoing.ready() { slot.outgoing.write(slot.sample) }
    }
    fn shot(index, pose, mode) {
        let slot = slots[index]
        if !slot.outgoing { return }
        let mode_id = 0.0
        if mode == "burst" { mode_id = 1.0 }
        if mode == "auto" { mode_id = 2.0 }
        slot.sent = slot.sent + 1
        slot.sample = {
            sequence = slot.sent mode = mode_id translation = pose.translation
            rotation = pose.rotation_quat_xyzw scale = pose.scale
        }
        slot.remaining = 0.24
        send(slot)
    }
    fn connect_slot(slot, group, address, generation) {
        let name = options.name + ".shot." + slot.index
        let outgoing = options.socket.publish(name).to(address).type(kind)
        let incoming = options.socket.subscribe(name).from(address).type(kind)
        group.attach(outgoing)
        group.attach(incoming)
        slot.outgoing = outgoing
        slot.seen = 0
        slot.sample = null
        on(incoming, "UdpSample", fn(event) {
            let sample = event.value
            if generation != link.generation || sample.sequence <= slot.seen { return }
            slot.seen = sample.sequence
            options.play(slot.index, {
                translation = sample.translation rotation_quat_xyzw = sample.rotation scale = sample.scale
            }, sample.mode)
        })
    }
    fn connect(group, address) {
        link.generation = link.generation + 1
        for slot in slots { connect_slot(slot, group, address, link.generation) }
    }
    fn disconnect() {
        link.generation = link.generation + 1
        for slot in slots { slot.outgoing = null slot.sample = null }
    }
    on_global("FrameTick", fn(event) {
        link.elapsed = link.elapsed + event.dt_sec
        if link.elapsed < 0.05 { return }
        let elapsed = link.elapsed
        link.elapsed = 0.0
        for slot in slots {
            if slot.sample {
                slot.remaining = slot.remaining - elapsed
                if slot.remaining <= 0.0 { slot.sample = null }
                else { send(slot) }
            }
        }
    })
    return { shot = shot connect = connect disconnect = disconnect }
}
