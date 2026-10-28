// A reusable motion controller, using existing Velocity and FrameTick APIs.
//
// let avatar_root = T.position(0, floor_y, 0) { GLTF("assets/components/characters/lumae/lumae.glb") {} }
// let pacing = pacing_and_waiting_around(avatar_root, false)
// pacing                         // emit/attach the returned Velocity wrapper
// pacing.start()                 // construction alone does not start movement
// pacing.stop()                  // stop before removing the avatar/controller
//
// Options (pass false for defaults): speed=0.35, walk_seconds=3.0,
// wait_seconds=2.0, turn_seconds=1.2, turn_radians=PI, ease_seconds=0.35.
// Speed is in the locomotion parent's units/second. Root forward is local +Z;
// the root and its parent must be upright (yaw-only rotation).
// The wrapper owns planar velocity and yaw rate. Gravity keeps ownership of Y.
// This moves the root; an eligible, calibrated AVC must supply walking IK.
// Plain GLTF NPCs do not automatically acquire AVC/IK by importing this module.
export fn pacing_and_waiting_around(avatar_root, options) {
    let speed = 0.35
    let walk_seconds = 3.0
    let wait_seconds = 2.0
    let turn_seconds = 1.2
    let turn_radians = 3.141592653589793
    let ease_seconds = 0.35
    if options {
        if options["speed"] != null { speed = options.speed }
        if options["walk_seconds"] != null { walk_seconds = options.walk_seconds }
        if options["wait_seconds"] != null { wait_seconds = options.wait_seconds }
        if options["turn_seconds"] != null { turn_seconds = options.turn_seconds }
        if options["turn_radians"] != null { turn_radians = options.turn_radians }
        if options["ease_seconds"] != null { ease_seconds = options.ease_seconds }
    }
    // Keep durations positive and cap the ease interval at half a walking leg.
    if speed < 0.0 { speed = 0.0 }
    if walk_seconds < 0.05 { walk_seconds = 0.05 }
    if wait_seconds < 0.05 { wait_seconds = 0.05 }
    if turn_seconds < 0.05 { turn_seconds = 0.05 }
    if ease_seconds < 0.001 { ease_seconds = 0.001 }
    if ease_seconds > walk_seconds * 0.5 { ease_seconds = walk_seconds * 0.5 }

    let state = { running = false phase = "waiting" elapsed = 0.0 turn_sign = 1.0 }
    let controller = Velocity { avatar_root }

    fn smooth(value) {
        let t = value
        if t < 0.0 { t = 0.0 }
        if t > 1.0 { t = 1.0 }
        return t * t * t * (10.0 + t * (-15.0 + 6.0 * t))
    }
    fn command(forward_speed, yaw_rate) {
        let q = avatar_root.trs().rotation_quat_xyzw
        // Rotate local +Z by the root's current local quaternion, into the
        // Velocity parent's frame. No world-yaw accumulator or pose overwrite.
        let x = 2.0 * (q[0] * q[2] + q[3] * q[1])
        let z = 1.0 - 2.0 * (q[0] * q[0] + q[1] * q[1])
        let length = Math.sqrt(x * x + z * z)
        if length < 0.0001 { length = 1.0 }
        let linear = controller.linear()
        let angular = controller.angular()
        // translate/rotate ADD velocity; subtract the live state to avoid
        // acceleration accumulation. Leave vertical velocity untouched.
        controller.translate([x / length * forward_speed - linear[0], 0.0, z / length * forward_speed - linear[2]])
        controller.rotate([-angular[0], yaw_rate - angular[1], -angular[2]])
    }
    controller.start = fn() {
        // Idempotent: repeated start calls do not restart a step/turn.
        state.running = true
    }
    controller.stop = fn() {
        state.running = false
        state.phase = "waiting"
        state.elapsed = 0.0
        state.turn_sign = 1.0
        command(0.0, 0.0)
    }
    controller.status = fn() {
        // Return a snapshot so callers cannot edit the controller's state.
        return { running = state.running phase = state.phase elapsed = state.elapsed }
    }

    on_global("FrameTick", fn(event) {
        if !state.running { return }
        let dt = event.dt_sec
        // Velocity integrates at most eight 120 Hz steps per presentation.
        // Freeze the schedule and stop on a hitch instead of fast-forwarding
        // a turn farther than the physical integrator can actually apply.
        if dt <= 0.0 || dt > 0.066666667 {
            command(0.0, 0.0)
            return
        }
        if state.phase == "waiting" {
            command(0.0, 0.0)
            state.elapsed = state.elapsed + dt
            if state.elapsed >= wait_seconds {
                state.elapsed = 0.0
                state.phase = "turning"
            }
        } else if state.phase == "turning" {
            let next = state.elapsed + dt
            if next > turn_seconds { next = turn_seconds }
            // Integrate the easing curve over this frame. Endpoints have zero
            // angular speed; using only its derivative would accumulate drift.
            let delta = smooth(next / turn_seconds) - smooth(state.elapsed / turn_seconds)
            command(0.0, turn_radians * state.turn_sign * delta / dt)
            state.elapsed = next
            if next >= turn_seconds {
                state.elapsed = 0.0
                state.phase = "walking"
            }
        } else {
            let next = state.elapsed + dt
            if next > walk_seconds { next = walk_seconds }
            let mid = (state.elapsed + next) * 0.5
            let envelope = smooth(mid / ease_seconds) * smooth((walk_seconds - mid) / ease_seconds)
            command(speed * envelope * (next - state.elapsed) / dt, 0.0)
            state.elapsed = next
            if next >= walk_seconds {
                state.elapsed = 0.0
                state.phase = "waiting"
                state.turn_sign = -state.turn_sign
            }
        }
    })
    return controller
}
