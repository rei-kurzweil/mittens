import { emissive_surface } from "../materials/emissive_surface.mms"
import { f32 } from "types"

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

let nozzle_z = -0.17

fn profile(width, length, speed, source_radius, tail_radius, opacity) {
    return {
        width = width length = length
        trail = {
            lifetime = 0.24 speed = speed opacity = opacity edge_wobble = 0.07
            offset = [0.0, 0.0, nozzle_z] flow_wavelength = 0.36 stop_fade = 0.15
            source_radius = source_radius tail_radius = tail_radius
        }
    }
}
let hover = profile(0.075, 0.18, 1.2, 0.05, 0.17, 0.48)
let thrust = profile(flame_width, flame_length, 3.9, 0.07, 0.34, 0.72)
let definition = MaterialDefinition(MaterialBase())
    .fragment("assets/shaders/curved-rocket-exhaust.frag")
    .inputs({ wave_amplitude: f32 })
let material = { smoke = null }

fn plume_material() {
    if !material.smoke {
        let smoke = Material(definition).inputs({ wave_amplitude = 0.08 })
        smoke
        material.smoke = smoke
    }
    return material.smoke
}

// The loose round hangs nose-down in the thigh slot. Its local -Z points out
// from the lower tip. The trail stays in world space as the nozzle turns.
export fn loose_rocket_exhaust(round) {
    let state = { flame = null mode = null }

    fn activate(mode) {
        if mode == null { mode = "thrust" }
        if state.mode == mode { return }
        let style = thrust
        if mode == "hover" { style = hover }
        if !state.flame {
            let flame = rocket_flame("sta_rocket_thrust_flame", nozzle_z, -1.0)
            round.attach(flame)
            state.flame = flame
        }
        let width = style.width / flame_width
        state.flame.update_transform(
            [0.0, 0.0, nozzle_z - 0.5 * style.length],
            [0.0, Math.pi, 0.0],
            [width, width, style.length / flame_length],
        )
        round.set_tube_trail(plume_material(), style.trail)
        state.mode = mode
    }

    fn deactivate() {
        if !state.flame { return }
        state.flame.remove_subtree()
        round.set_tube_trail()
        state.flame = null
        state.mode = null
    }

    return { activate = activate deactivate = deactivate }
}
