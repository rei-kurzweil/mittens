import { f32, vec4 } from "types"

// Each weapon has a fixed blast radius. A sample carries up to four
// consecutive impacts so the latest-value transport can recover a burst.
export fn shared_impact(options) {
    let kind = { first: f32, count: f32, a: vec4, b: vec4, c: vec4, d: vec4 }
    let empty = [0.0, 0.0, 0.0, 0.0]
    let state = { generation = 0 outgoing = null queue = {} first = 1 next = 1 received = 0 elapsed = 0.0 pump = null }

    fn queue_key(sequence) { return "" + (sequence % 64) }
    fn queued(sequence) {
        if sequence >= state.next { return empty }
        return state.queue[queue_key(sequence)]
    }

    fn send() {
        if !state.outgoing || !state.outgoing.ready() || state.first == state.next { return }
        let first = state.first
        let count = Math.clamp(state.next - first, 0, 4)
        state.outgoing.write({
            first = first count = count
            a = queued(first) b = queued(first + 1) c = queued(first + 2) d = queued(first + 3)
        })
    }

    fn hit(position, amount) {
        options.apply(position, amount)
        if !state.outgoing || state.next - state.first >= 64 { return }
        state.queue[queue_key(state.next)] = [position[0], position[1], position[2], amount]
        state.next = state.next + 1
        if state.next == state.first + 1 { state.pump.bypass(false) }
        send()
    }

    fn disconnect() {
        state.generation = state.generation + 1
        state.outgoing = null
        state.queue = {}
        state.first = 1
        state.next = 1
        state.received = 0
        state.elapsed = 0.0
        state.pump.bypass(true)
    }

    fn connect(group, address) {
        disconnect()
        let generation = state.generation
        let impact_name = options.name + ".impact"
        let ack_name = impact_name + ".ack"
        let outgoing = options.socket.publish(impact_name).to(address).type(kind)
        let incoming = options.socket.subscribe(impact_name).from(address).type(kind)
        let ack = options.socket.publish(ack_name).to(address).type(f32)
        let acknowledged = options.socket.subscribe(ack_name).from(address).type(f32)
        group.attach(outgoing)
        group.attach(incoming)
        group.attach(ack)
        group.attach(acknowledged)
        state.outgoing = outgoing
        on(incoming, "UdpSample", fn(event) {
            if generation != state.generation { return }
            let sample = event.value
            let slots = [sample.a, sample.b, sample.c, sample.d]
            for i in range(Math.clamp(Math.floor(sample.count), 0, 4)) {
                let sequence = sample.first + i
                if sequence == state.received + 1 {
                    let impact = slots[i]
                    options.apply([impact[0], impact[1], impact[2]], impact[3])
                    state.received = sequence
                }
            }
            if ack.ready() { ack.write(state.received) }
        })
        on(acknowledged, "UdpSample", fn(event) {
            if generation != state.generation { return }
            let last = Math.floor(event.value)
            if last < state.first || last >= state.next { return }
            for i in range(last - state.first + 1) { state.queue[queue_key(state.first + i)] = null }
            state.first = last + 1
            if state.first == state.next { state.pump.bypass(true) }
            send()
        })
    }

    state.pump = on(world, "FrameTick", fn(event) {
        state.elapsed = state.elapsed + event.dt_sec
        if state.elapsed < 0.05 { return }
        state.elapsed = 0.0
        send()
    })
    state.pump.bypass(true)
    return { hit = hit connect = connect disconnect = disconnect }
}
