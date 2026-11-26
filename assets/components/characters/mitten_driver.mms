// The truck faces -Z. Start turning outside the front and side windows,
// then keep turning until the target is comfortably inside them.
export fn mitten_driver(ride, player) {
    let state = { turn = false steering = 0.0 throttle = 0.0 }
    let bounds = ride["turn_bounds"]
    fn stop() {
        if ride.truck && state.turn { ride.truck.set_control_input({ brake = true }) }
        state.turn = false
        state.steering = 0.0
        state.throttle = 0.0
    }
    fn bearing_to_player() {
        let pose = ride.truck.world_trs()
        let q = pose.rotation_quat_xyzw
        let fx = -2.0 * (q[0] * q[2] + q[3] * q[1])
        let fz = -(1.0 - 2.0 * (q[0] * q[0] + q[1] * q[1]))
        let p = player.world_trs().translation
        let dx = p[0] - pose.translation[0]
        let dz = p[2] - pose.translation[2]
        let angle = Math.atan2(fx * dz - fz * dx, fx * dx + fz * dz)
        return { angle = angle position = pose.translation forward_x = fx forward_z = fz }
    }
    fn update() {
        if !ride.truck || ride.truck.drive_status().mounted { state.turn = false return }
        let bearing = bearing_to_player()
        let limit = 80.0
        if state.turn { limit = 48.0 } // Allow for a little drift under braking.
        if Math.abs(bearing.angle) <= limit * Math.pi / 180.0 { stop() return }
        let sign = 1.0
        if bearing.angle < 0.0 { sign = -1.0 }
        let throttle = 0.72
        // Reverse an arc if the forward arc heads beyond the scene's driving area.
        let x = bearing.position[0] + bearing.forward_x * 5.0
        let z = bearing.position[2] + bearing.forward_z * 5.0
        if bounds && (x < bounds.min_x || x > bounds.max_x || z < bounds.min_z || z > bounds.max_z) {
            throttle = -throttle
            sign = -sign
        }
        if state.turn && throttle == state.throttle && sign == state.steering { return }
        state.turn = true
        state.throttle = throttle
        state.steering = sign
        ride.truck.set_control_input({ throttle = throttle steering = sign brake = false })
    }
    return { update = update stop = stop turning = fn() { return state.turn }
        can_fire = fn() {
            return !state.turn && ride.truck && Math.abs(bearing_to_player().angle) <= 80.0 * Math.pi / 180.0
        } }
}
