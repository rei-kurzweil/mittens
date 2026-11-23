import { emissive_surface } from "../materials/emissive_surface.mms"
import { smoke } from "../smoke.mms"

let flame_width = 0.114
let flame_length = 0.48

export fn rocket_flame(name, origin_z, direction) {
    let rotation = 0.0
    if direction < 0 { rotation = Math.pi }
    return T.position(0.0, 0.0, origin_z + direction * flame_length * 0.5).rotation(0.0, rotation, 0.0) {
        name = name
        Raycastable.disabled()
        emissive_surface()
        Emissive.on().intensity(3.0)
        T.scale(flame_width, flame_width, flame_length) {
            R.cone() { C.rgba(1.0, 0.45, 0.04, 1.0) }
        }
    }
}

// A loose round hangs nose-down in the thigh slot. Emit at its lower tip.
// The pool stays in world space as the round and its flame move away.
export fn loose_rocket_exhaust(round) {
    let state = { flame = null trail = null source = null tick = null }

    fn deactivate() {
        if !state.flame { return }
        if state.flame.is_alive() { state.flame.remove_subtree() }
        state.flame = null
        state.trail.stop()
    }

    fn update(event) {
        if !round.is_alive() { deactivate() }
        state.trail.update(event.dt_sec)
        if state.flame || state.trail.stats().active > 0 { return }
        state.tick.remove()
        state.trail.dispose()
        if state.source.is_alive() { state.source.remove_subtree() }
        state.trail = null
        state.tick = null
        state.source = null
    }

    fn activate() {
        if state.flame { return }
        if !state.trail {
            let source = T.position(0.0, 0.0, -0.48) {}
            round.attach(source)
            let trail = smoke({
                name = "sta_rocket_thrust_smoke" source = source
                capacity = 8 lifetime = 0.48 interval = 0.075 size = 0.09 growth = 4.0
                velocity = [0.0, -1.3, 0.0] acceleration = [0.0, 0.12, 0.0]
                spread = 0.2 inherit_velocity = 0.0 wobble = 0.03 max_opacity = 0.58
            })
            trail.root
            state.source = source
            state.trail = trail
            state.tick = on(world, "FrameTick", update)
        }
        let flame = rocket_flame("sta_rocket_thrust_flame", -0.17, -1.0)
        round.attach(flame)
        state.flame = flame
        state.trail.start()
    }

    return { activate = activate deactivate = deactivate }
}
