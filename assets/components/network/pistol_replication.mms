import { vec2, vec3, vec4 } from "types"

// Carry all three cadence slots in one latest-value sample so later shots
// cannot replace the other slots before the receiver polls.
export fn pistol_replication(options) {
    let kind = {
        m0: vec2, p0: vec3, q0: vec4, s0: vec3,
        m1: vec2, p1: vec3, q1: vec4, s1: vec3,
        m2: vec2, p2: vec3, q2: vec4, s2: vec3,
    }
    let empty = { meta = [0,0] translation = [0,0,0] rotation = [0,0,0,1] scale = [1,1,1] }
    let link = { generation = 0 elapsed = 0.0 outgoing = null pump = null }
    fn make_slot() {
        return { sent = 0 seen = 0 sample = null remaining = 0.0 }
    }
    let slots = [make_slot(), make_slot(), make_slot()]

    fn send() {
        if !link.outgoing || !link.outgoing.ready() { return }
        let snapshot = {}
        for index in range(3) {
            let sample = slots[index].sample
            if !sample { sample = empty }
            snapshot["m" + index] = sample.meta
            snapshot["p" + index] = sample.translation
            snapshot["q" + index] = sample.rotation
            snapshot["s" + index] = sample.scale
        }
        link.outgoing.write(snapshot)
    }
    fn shot(index, pose, mode) {
        if !link.outgoing { return }
        let slot = slots[index]
        let mode_id = 0.0
        if mode == "burst" { mode_id = 1.0 }
        if mode == "auto" { mode_id = 2.0 }
        slot.sent = slot.sent + 1
        slot.sample = {
            meta = [slot.sent, mode_id] translation = pose.translation
            rotation = pose.rotation_quat_xyzw scale = pose.scale
        }
        slot.remaining = 0.24
        link.pump.bypass(false)
        send()
    }
    fn disconnect() {
        link.generation = link.generation + 1
        link.elapsed = 0.0
        link.outgoing = null
        for slot in slots {
            slot.seen = 0
            slot.sample = null
        }
        link.pump.bypass(true)
    }
    fn connect(group, address) {
        disconnect()
        let generation = link.generation
        let name = options.name + ".shots"
        let outgoing = options.socket.publish(name).to(address).type(kind)
        let incoming = options.socket.subscribe(name).from(address).type(kind)
        group.attach(outgoing)
        group.attach(incoming)
        link.outgoing = outgoing
        on(incoming, "UdpSample", fn(event) {
            if generation != link.generation { return }
            let sample = event.value
            for index in range(3) {
                let slot = slots[index]
                let meta = sample["m" + index]
                if meta[0] > slot.seen {
                    slot.seen = meta[0]
                    options.play(index, {
                        translation = sample["p" + index]
                        rotation_quat_xyzw = sample["q" + index]
                        scale = sample["s" + index]
                    }, meta[1])
                }
            }
        })
    }
    link.pump = on_global("FrameTick", fn(event) {
        link.elapsed = link.elapsed + event.dt_sec
        if link.elapsed < 0.05 { return }
        let elapsed = link.elapsed
        link.elapsed = 0.0
        let pending = false
        for slot in slots {
            if slot.sample {
                slot.remaining = slot.remaining - elapsed
                if slot.remaining <= 0.0 { slot.sample = null }
                else { pending = true }
            }
        }
        if pending { send() } else { link.pump.bypass(true) }
    })
    link.pump.bypass(true)
    return { shot = shot connect = connect disconnect = disconnect }
}
