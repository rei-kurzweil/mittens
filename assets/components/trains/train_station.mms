import { f32, vec4 } from "types"

// The host owns the train's yaw and reversals. A connected client repeats
// its toggle count until acknowledged, so dropped samples cannot lose a stop.
export fn train_station(pivot, motion, speed, center_z, host) {
    let caption = Text { "Pause train" }
    let button = T.position(0.0, 1.1, 0.0) {
        name = "train_station_button"
        T.scale(0.78, 0.32, 0.10) {
            R.cube() { C.rgba(0.17, 0.32, 0.27, 1.0) Raycastable.click_only() { interaction_priority(180) } }
        }
        T.position(-0.30, -0.045, 0.06).scale(0.08, 0.08, 0.08) { Raycastable.disabled() caption }
    }
    let state = { paused = false heading = speed out = null sequence = 0 seen = 0 requests = 0 ack = 0 elapsed = 0 }
    fn drive() {
        let goal = state.heading
        if state.paused { goal = 0.0 }
        let delta = goal - motion.angular()[1]
        if delta != 0.0 { motion.rotate([0.0, delta, 0.0]) }
    }
    fn pause(value) {
        if state.paused != value {
            state.paused = value
            if value { caption.set_text("Resume train") } else { caption.set_text("Pause train") }
        }
        drive()
    }
    fn send() {
        if !state.out || !state.out.ready() { return }
        if host {
            state.sequence = state.sequence + 1
            let stopped = 0
            if state.paused { stopped = 1 }
            state.out.write({ seq = state.sequence ack = state.ack paused = stopped heading = state.heading rotation = pivot.trs().rotation_quat_xyzw })
        } else if state.requests > state.ack { state.out.write(state.requests) }
    }
    on(button, "Click", fn(event) {
        if host || !state.out { pause(!state.paused) } else { state.requests = state.requests + 1 }
        send()
    })
    on(motion, "ZoneContact", fn(event) {
        if !host && state.out { return }
        state.heading = speed
        if event.point[2] < center_z { state.heading = -speed }
        drive()
    })
    drive()
    fn connect(socket, group, address) {
        let kind = { seq: f32, ack: f32, paused: f32, heading: f32, rotation: vec4 }
        state.requests = 0
        state.ack = 0
        state.seen = 0
        if host {
            state.out = socket.publish("train.state").to(address).type(kind)
            let request = socket.subscribe("train.toggle").from(address).type(f32)
            group.attach(request)
            on(request, "UdpSample", fn(event) {
                if event.value <= state.ack { return }
                let count = event.value - state.ack
                state.ack = event.value
                if count - 2 * Math.floor(count / 2) == 1 { pause(!state.paused) }
                send()
            })
        } else {
            state.out = socket.publish("train.toggle").to(address).type(f32)
            let incoming = socket.subscribe("train.state").from(address).type(kind)
            group.attach(incoming)
            on(incoming, "UdpSample", fn(event) {
                let value = event.value
                if value.seq <= state.seen { return }
                state.seen = value.seq
                state.ack = value.ack
                pivot.set_rotation_quat(value.rotation, "local")
                state.heading = value.heading
                pause(value.paused > 0.5)
            })
        }
        group.attach(state.out)
        send()
    }
    fn tick(dt) {
        if !state.out { return }
        state.elapsed = state.elapsed + dt
        if state.elapsed >= 0.20 { state.elapsed = 0 send() }
    }
    let root = T {
        name = "east_train_pedestal"
        T.position(0.0, 0.52, 0.0).scale(0.22, 1.04, 0.22) { R.cube() { C.rgba(0.36, 0.38, 0.44, 1.0) } }
        button
    }
    return { root = root connect = connect disconnect = fn() { state.out = null } tick = tick }
}
