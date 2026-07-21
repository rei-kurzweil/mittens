// Reusable studio light fixture.
//
// `mounted_light` may be any light component (AL, DL, PL, SL) or may be omitted.
// An empty fixture has no luminous face, making its appearance match its behavior.
export fn tripod_light(light_name, position, look_target, mounted_light) {
    return T.position(position[0], position[1], position[2]) {
        name = light_name

        // The fixture origin is the ground plane under its feet. Account for
        // both the leg's length and thickness when finding its rotated Y extent.
        let leg_length = 0.78
        let leg_thickness = 0.09
        let leg_pitch = -0.62
        let leg_vertical_height =
            leg_length * Math.cos(leg_pitch) +
            leg_thickness * Math.abs(Math.sin(leg_pitch))
        let leg_center_y = leg_vertical_height / 2.0

        // Three identical splayed legs, evenly spaced around the vertical stand.
        let leg_spacing = 2.0 * Math.pi / 3.0
        for i in range(3) {
            T.rotation(0.0, i * leg_spacing, 0.0) {
                T.position(0.0, leg_center_y, 0.32)
                    .rotation(leg_pitch, 0.0, 0.0)
                    .scale(leg_thickness, leg_length, leg_thickness) {
                    R.cube() { C.rgba(0.20, 0.22, 0.27, 1.0) }
                }
            }
        }

        // Stack the shaft on the legs, then place the rotating head at its top.
        let shaft_height = 2.25
        let shaft_center_y = leg_vertical_height + shaft_height / 2.0
        let head_y = leg_vertical_height + shaft_height
        T.position(0.0, shaft_center_y, 0.0).scale(0.10, shaft_height, 0.10) {
            name = "tripod_light_shaft"
            R.cube() { C.rgba(0.16, 0.18, 0.22, 1.0) }
        }

        T.position(0.0, head_y, 0.0).looking_at(look_target) {
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
