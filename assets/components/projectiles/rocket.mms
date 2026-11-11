// First flight smoke test: native Velocity owns movement after acceleration.
export fn rocket(spec) {
    let state = { speed = 2.0 alive = true }
    let bindings = { acceleration = null }
    let model = T.position(0.191356, -1.507589, -0.111893) {
        GLTF.new("assets/models/STA_rocket_bullet.glb") {}
    }
    let motion = Velocity { model }
    let root = T {
        name = "sta_rocket_flight"
        Raycastable.disabled()
        motion
    }
    root
    root.world_trs({ translation = spec.pose.translation rotation_quat_xyzw = spec.pose.rotation_quat_xyzw scale = [1.0, 1.0, 1.0] })
    motion.translate([0.0, 0.0, -state.speed])
    bindings.acceleration = on(world, "FrameTick", fn(event) {
        if state.alive {
            let next = Math.min(18.0, state.speed + 12.0 * event.dt_sec)
            motion.translate([0.0, 0.0, state.speed - next])
            state.speed = next
            if state.speed >= 18.0 { bindings.acceleration.remove() }
        }
    })
    let expiry = Animation.paused().length(12.0) {
        Keyframe.at(0.0) {}
        Keyframe.at(12.0) {
            state.alive = false
            bindings.acceleration.remove()
            root.remove_subtree()
        }
    }
    root.attach(expiry)
    expiry.play()
    return root
}
