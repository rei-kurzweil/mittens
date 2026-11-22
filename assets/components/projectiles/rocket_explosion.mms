import { emissive_surface } from "../materials/emissive_surface.mms"

let burst_duration = 0.5
let outer_delay = 0.4
let outer_opacity = 2.0 / 3.0
let inner_material = MaterialDefinition.toon()

fn burst_size(progress) {
    let remaining = 1.0 - progress
    return 1.5 - (1.5 - 0.04) * remaining * remaining
}

fn outer_burst() {
    return T.scale(0.16, 0.16, 0.08) {
        name = "rocket_explosion_outer"
        R.sphere() {
            emissive_surface()
            C.rgba(1.0, 1.0, 1.0, outer_opacity)
            Emissive.on().intensity(1.0)
        }
    }
}

export fn rocket_explosion(position, rotation) {
    if !rotation { rotation = [0.0, 0.0, 0.0, 1.0] }
    let inner = T.scale(0.04, 0.04, 0.04) {
        name = "rocket_explosion_inner"
        R.sphere() {
            Material(inner_material)
            C.rgba(1.0, 0.65, 0.12, 0.75)
            Emissive.on().intensity(5.0)
        }
    }
    let root = T.position(position[0], position[1], position[2])
        .rotation_quat(rotation[0], rotation[1], rotation[2], rotation[3]) {
        name = "rocket_explosion"
        Raycastable.disabled()
        inner
    }
    root

    // The effect lasts in seconds even in scenes with a different animation tempo.
    let state = { elapsed = 0.0 inner = inner outer = null tick = null }
    state.tick = on(world, "FrameTick", fn(event) {
        state.elapsed = state.elapsed + event.dt_sec
        if state.elapsed >= outer_delay + burst_duration {
            state.tick.remove()
            root.remove_subtree()
            return
        }
        if state.inner {
            if state.elapsed >= burst_duration {
                state.inner.remove_subtree()
                state.inner = null
            } else {
                let size = burst_size(state.elapsed / burst_duration)
                inner.update_transform([0, 0, 0], [0, 0, 0], [size, size, size])
            }
        }
        if state.elapsed < outer_delay { return }
        if !state.outer {
            let outer = outer_burst()
            root.attach(outer)
            state.outer = outer
        }
        let progress = (state.elapsed - outer_delay) / burst_duration
        let size = burst_size(progress)
        let blue = 1.0 - Math.clamp(2.0 * progress, 0.0, 1.0)
        let brightness = 1.0 - Math.clamp(2.0 * progress - 1.0, 0.0, 1.0)
        state.outer.update_transform([0, 0, 0], [0, 0, 0], [4.0 * size, 4.0 * size, 2.0 * size])
        state.outer.set_color([brightness, brightness, blue, outer_opacity * (1.0 - progress)])
    })
}
