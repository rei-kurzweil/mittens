// Locally pause each secondary-motion root when solid cover blocks the
// observer's view. This only affects the supplied avatar's model.
export fn secondary_motion_visibility(model, center, observer) {
    let state = { elapsed = 1.0 paused = false }
    let tick = on_global("FrameTick", fn(event) {
        state.elapsed = state.elapsed + event.dt_sec
        if state.elapsed < 1.0 { return }
        state.elapsed = 0.0
        let from = observer.world_trs().translation
        let to = center.world_trs().translation
        let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]]
        let distance = Math.sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])
        let blocked = false
        if distance > 1.2 {
            let start = [from[0] + d[0] * 0.6 / distance, from[1] + d[1] * 0.6 / distance,
                from[2] + d[2] * 0.6 / distance]
            let hit = Physics.raycast(start, d, distance - 0.6)
            blocked = hit != null && hit.distance < distance - 1.2
        }
        if blocked != state.paused {
            state.paused = blocked
            for motion in model.query_all("secondary_motion") { motion.set_paused(blocked) }
        }
    })
    return { remove = fn() { tick.remove() } }
}
