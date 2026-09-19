// Reusable horizontal three-aspect traffic light.
//
// The signal faces local +Z. Each lamp has a five-sided shade: black inward
// faces surround the emissive lens, with matching yellow outward faces. The
// open front keeps the lens visible.

fn signal_lens(x, color, signal_name) {
    return T.position(x, 0.0, 0.34) {
        name = signal_name

        // Emissive circular signal face.
        T.scale(0.38, 0.38, 1.0) {
            R.circle2d(0.5, 64) {
                C.rgba(color[0], color[1], color[2], 1.0)
                EM.on() { intensity(2.0) }
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

export fn traffic_light() {
    return T {
        name = "traffic_light"
        Grabbable {}

        // A yellow enclosure ties the three horizontal lamp shades together.
        T.position(0.0, 0.0, -0.38).scale(2.25, 0.82, 0.12) {
            name = "traffic_light_enclosure"
            R.cube() { C.rgba(0.98, 0.72, 0.08, 1.0) }
        }

        signal_lens(-0.72, [1.0, 0.08, 0.04], "traffic_light_red")
        signal_lens(0.0, [1.0, 0.72, 0.02], "traffic_light_yellow")
        signal_lens(0.72, [0.08, 1.0, 0.20], "traffic_light_green")
    }
}
