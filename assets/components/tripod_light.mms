// Reusable studio light fixture.
//
// `mounted_light` may be any light component (AL, DL, PL, SL) or may be omitted.
// An empty fixture has no luminous face, making its appearance match its behavior.
export fn tripod_light(light_name, position, look_target, mounted_light) {
    return T.position(position[0], position[1], position[2]) {
        name = light_name

        // Three splayed tripod legs and a vertical stand.
        T.position(-0.32, -1.78, 0.0).rotation(0.0, 0.0, -0.62).scale(0.09, 0.78, 0.09) {
            R.cube() { C.rgba(0.20, 0.22, 0.27, 1.0) }
        }
        T.position(0.32, -1.78, 0.0).rotation(0.0, 0.0, 0.62).scale(0.09, 0.78, 0.09) {
            R.cube() { C.rgba(0.20, 0.22, 0.27, 1.0) }
        }
        T.position(0.0, -1.78, 0.32).rotation(-0.62, 0.0, 0.0).scale(0.09, 0.78, 0.09) {
            R.cube() { C.rgba(0.20, 0.22, 0.27, 1.0) }
        }
        // The wider stance supports a shaft 25% shorter than the original 3-unit pole.
        T.position(0.0, 0.0, 0.0).scale(0.10, 2.25, 0.10) {
            name = "tripod_light_shaft"
            R.cube() { C.rgba(0.16, 0.18, 0.22, 1.0) }
        }

        T.position(0.0, 1.62, 0.0).looking_at(look_target) {
            // A compact rear mounting block sits over the shaft. The housing is
            // offset forward from it along the fixture's local +Z direction.
            T.position(0.0, 0.0, -0.12).scale(0.28, 0.28, 0.32) {
                name = "tripod_light_rear_mount"
                R.cube() { C.rgba(0.12, 0.13, 0.16, 1.0) }
            }

            T.position(0.0, 0.0, 0.12) {
                name = "tripod_light_housing"
                T.scale(0.78, 0.52, 0.22) {
                    R.cube() { C.rgba(0.075, 0.08, 0.105, 1.0) }
                }

                if mounted_light {
                    // Keep the emitter just beyond the housing face to avoid z-fighting.
                    T.position(0.0, 0.0, 0.116).scale(0.68, 0.42, 0.012) {
                        name = "tripod_light_emissive_face"
                        R.cube() {
                            C.rgba(1.0, 1.0, 1.0, 1.0)
                            EM.on() { intensity(2.5) }
                        }
                    }
                    mounted_light
                }
            }
        }
    }
}
