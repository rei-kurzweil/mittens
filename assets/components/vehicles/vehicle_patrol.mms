// A control script that drives a vehicle back and forth along a road through
// the common vehicle input (`vehicle.set_control_input`), so no occupant has
// to drive. One looping animation times it, with nothing per frame: drive
// toward whichever end of the road is farther, brake, drive back, brake.
// Pick the direction from where the vehicle is, so it never wanders off one
// end however far a run takes it.
//
// vehicle: has set_control_input({ throttle steering brake })
// frame:   the transform whose position and -Z facing are the vehicle's
// options: name, axis ("x" or "z", default "x"), center (default 0),
//          drive_seconds (3.0), brake_seconds (1.5), throttle (1.0),
//          slow_throttle (every other run uses it instead; default none),
//          beats_per_second (the scene Clock, 2.0 at 120 bpm)
export fn vehicle_patrol(vehicle, frame, options) {
    let axis = 0
    if options["axis"] == "z" { axis = 2 }
    let center = 0.0
    if options["center"] != null { center = options.center }
    let drive = 3.0
    if options["drive_seconds"] != null { drive = options.drive_seconds }
    let brake = 1.5
    if options["brake_seconds"] != null { brake = options.brake_seconds }
    let throttle = 1.0
    if options["throttle"] != null { throttle = options.throttle }
    let bps = 2.0
    if options["beats_per_second"] != null { bps = options.beats_per_second }
    let slow = throttle
    if options["slow_throttle"] != null { slow = options.slow_throttle }
    let state = { running = true runs = 0 }

    fn go() {
        if !state.running { return }
        let pose = frame.world_trs()
        let q = pose.rotation_quat_xyzw
        // World direction of the vehicle's -Z (its front).
        let forward = [
            -2.0 * (q[0] * q[2] + q[3] * q[1]),
            -2.0 * (q[1] * q[2] - q[3] * q[0]),
            -(1.0 - 2.0 * (q[0] * q[0] + q[1] * q[1])),
        ]
        let toward = 1.0
        if pose.translation[axis] > center { toward = -1.0 }
        let facing = 1.0
        if forward[axis] < 0.0 { facing = -1.0 }
        let amount = throttle
        if state.runs % 2 == 1 { amount = slow }
        state.runs = state.runs + 1
        vehicle.set_control_input({ throttle = amount * toward * facing steering = 0.0 brake = false })
    }
    fn stop() {
        if !state.running { return }
        vehicle.set_control_input({ throttle = 0.0 steering = 0.0 brake = true })
    }

    // Keyframes run once ahead for audio and again at visual time, so they
    // only emit events; the handler drives the vehicle once per step.
    let events = T { name = options.name + "_patrol_events" }
    let leg = (drive + brake) * bps
    let cycle = Animation.looping().length(2.0 * leg) {
        name = options.name + "_patrol"
        Keyframe.at(0.0) { emit_data(events, "PatrolGo") }
        Keyframe.at(drive * bps) { emit_data(events, "PatrolStop") }
        Keyframe.at(leg) { emit_data(events, "PatrolGo") }
        Keyframe.at(leg + drive * bps) { emit_data(events, "PatrolStop") }
    }
    on(events, "DataEvent", fn(event) {
        if event == "PatrolGo" { go() }
        if event == "PatrolStop" { stop() }
    })
    let root = T { name = options.name + "_patrol_root" events cycle }
    root.set_running = fn(value) {
        state.running = value == true
        if !state.running { vehicle.set_control_input({ brake = true }) }
    }
    return root
}
