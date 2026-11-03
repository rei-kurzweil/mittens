import { emissive_surface } from "./materials/emissive_surface.mms"

// Reusable horizontal three-aspect traffic light.
//
// The signal faces local +Z. Each lamp has a five-sided shade: black inward
// faces surround the emissive lens, with matching yellow outward faces. The
// open front keeps the lens visible.

fn signal_lens(x, color, signal_name, glow) {
    return T.position(x, 0.0, 0.34) {
        name = signal_name

        // Emissive circular signal face.
        T.scale(0.38, 0.38, 1.0) {
            R.circle_2d() {
                C.rgba(color[0], color[1], color[2], 1.0)
                emissive_surface()
                glow
            }
        }

        // Yellow visor above the lamp, pitched forward over its face.
        T.position(0.0, 0.38, 0.12)
            .rotation(1.10, 0.0, 0.0)
            .scale(0.50, 0.30, 1.0) {
            name = "traffic_light_hood"
            R.plane() { C.rgba(0.98, 0.72, 0.08, 1.0) }
        }

        // Five black planes face into the shade: rear, top, bottom, and sides.
        T.position(0.0, 0.0, -0.33).scale(0.50, 0.50, 1.0) {
            R.plane() { C.rgba(0.025, 0.025, 0.030, 1.0) }
        }
        T.position(0.0, 0.31, -0.08).rotation(1.5708, 0.0, 0.0).scale(0.50, 0.34, 1.0) {
            R.plane() { C.rgba(0.025, 0.025, 0.030, 1.0) }
        }
        T.position(0.0, -0.31, -0.08).rotation(-1.5708, 0.0, 0.0).scale(0.50, 0.34, 1.0) {
            R.plane() { C.rgba(0.025, 0.025, 0.030, 1.0) }
        }
        T.position(-0.31, 0.0, -0.08).rotation(0.0, -1.5708, 0.0).scale(0.34, 0.50, 1.0) {
            R.plane() { C.rgba(0.025, 0.025, 0.030, 1.0) }
        }
        T.position(0.31, 0.0, -0.08).rotation(0.0, 1.5708, 0.0).scale(0.34, 0.50, 1.0) {
            R.plane() { C.rgba(0.025, 0.025, 0.030, 1.0) }
        }

        // Matching planes form the non-emissive yellow exterior.
        T.position(0.0, 0.0, -0.35).rotation(0.0, 3.14159, 0.0).scale(0.52, 0.52, 1.0) {
            R.plane() { C.rgba(0.98, 0.72, 0.08, 1.0) }
        }
        T.position(0.0, 0.33, -0.08).rotation(-1.5708, 0.0, 0.0).scale(0.52, 0.35, 1.0) {
            R.plane() { C.rgba(0.98, 0.72, 0.08, 1.0) }
        }
        T.position(0.0, -0.33, -0.08).rotation(1.5708, 0.0, 0.0).scale(0.52, 0.35, 1.0) {
            R.plane() { C.rgba(0.98, 0.72, 0.08, 1.0) }
        }
        T.position(-0.33, 0.0, -0.08).rotation(0.0, 1.5708, 0.0).scale(0.35, 0.52, 1.0) {
            R.plane() { C.rgba(0.98, 0.72, 0.08, 1.0) }
        }
        T.position(0.33, 0.0, -0.08).rotation(0.0, -1.5708, 0.0).scale(0.35, 0.52, 1.0) {
            R.plane() { C.rgba(0.98, 0.72, 0.08, 1.0) }
        }
    }
}

export fn traffic_light(initial_state, fixed) {
    let red = EM.on().intensity(2.0)
    let yellow = EM.on().intensity(2.0)
    let green = EM.on().intensity(2.0)
    let signal = T {
        name = "traffic_light"
        if fixed { Raycastable.disabled() } else { Grabbable {} }

        // A yellow enclosure ties the three horizontal lamp shades together.
        T.position(0.0, 0.0, -0.38).scale(2.25, 0.82, 0.12) {
            name = "traffic_light_enclosure"
            R.cube() { C.rgba(0.98, 0.72, 0.08, 1.0) }
        }

        signal_lens(-0.72, [1.0, 0.08, 0.04], "traffic_light_red",red)
        signal_lens(0.0, [1.0, 0.72, 0.02], "traffic_light_yellow",yellow)
        signal_lens(0.72, [0.08, 1.0, 0.20], "traffic_light_green",green)
    }
    signal.set_state = fn(value) {
        let r=0.04 let y=0.04 let g=0.04
        if value == "red" || value == null { r=2.0 }
        if value == "yellow" || value == null { y=2.0 }
        if value == "green" || value == null { g=2.0 }
        red.set_intensity(r) yellow.set_intensity(y) green.set_intensity(g)
    }
    signal.set_state(initial_state)
    return signal
}
