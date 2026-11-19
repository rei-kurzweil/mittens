// Visual-only straight railroad. The track never contributes collision: the
// train's future sweeping zone should contact buildings, not its own rails.
//
// Local +Z is the track direction. `base_height` is the top of the rail heads,
// so a vehicle root can sit at the same local Y. Two separate CombineMesh
// groups keep the wooden sleepers and the steel rails independently batchable.
export fn railroad_track(label, length, gauge, sleeper_spacing, base_height, sleeper_color, rail_color, material) {
    let rail_height = 0.12
    let rail_thickness = 0.14
    let sleeper_height = 0.14
    let sleeper_width = gauge + 0.90
    let sleeper_depth = 0.24
    let sleeper_count = Math.floor(length / sleeper_spacing) + 1

    return T.position(0.0, base_height, 0.0) {
        name = label

        CombineMesh {
            name = label + "_wooden_sleepers"
            if material != null { material }
            for index in range(sleeper_count) {
                let z = (index - (sleeper_count - 1) / 2.0) * sleeper_spacing
                T.position(0.0, -rail_height - sleeper_height / 2.0, z)
                    .scale(sleeper_width, sleeper_height, sleeper_depth) {
                    R.cube() {
                        C.rgba(sleeper_color[0], sleeper_color[1], sleeper_color[2], 1.0)
                    }
                }
            }
        }

        CombineMesh {
            name = label + "_rails"
            if material != null { material }
            for side in [-1.0, 1.0] {
                T.position(side * gauge / 2.0, -rail_height / 2.0, 0.0)
                    .scale(rail_thickness, rail_height, length) {
                    R.cube() {
                        C.rgba(rail_color[0], rail_color[1], rail_color[2], 1.0)
                    }
                }
            }
        }
    }
}

// Visual-only circular arc of track around the local origin. Angles are yaw in
// radians; angle 0 is local +X and positive angles turn toward -Z, matching a
// positive Y angular velocity. Sleepers lie radially; rails are short chords.
export fn railroad_arc(label, radius, start_angle, end_angle, gauge, sleeper_spacing, base_height, sleeper_color, rail_color, material) {
    let rail_height = 0.12
    let rail_thickness = 0.14
    let sleeper_height = 0.14
    let sleeper_width = gauge + 0.90
    let sleeper_depth = 0.24
    let sweep = end_angle - start_angle
    let sleeper_count = Math.floor(Math.abs(sweep) * radius / sleeper_spacing) + 1
    let chord_count = Math.floor(Math.abs(sweep) * radius / 1.0) + 1
    let chord_angle = sweep / chord_count

    return T.position(0.0, base_height, 0.0) {
        name = label

        CombineMesh {
            name = label + "_wooden_sleepers"
            if material != null { material }
            for index in range(sleeper_count) {
                let angle = start_angle + sweep * (index + 0.5) / sleeper_count
                T.rotation(0.0, angle, 0.0) {
                    T.position(radius, -rail_height - sleeper_height / 2.0, 0.0)
                        .scale(sleeper_width, sleeper_height, sleeper_depth) {
                        R.cube() {
                            C.rgba(sleeper_color[0], sleeper_color[1], sleeper_color[2], 1.0)
                        }
                    }
                }
            }
        }

        CombineMesh {
            name = label + "_rails"
            if material != null { material }
            for side in [-1.0, 1.0] {
                let rail_radius = radius + side * gauge / 2.0
                let length = 2.0 * rail_radius * Math.sin(Math.abs(chord_angle) / 2.0) + 0.02
                for index in range(chord_count) {
                    let angle = start_angle + chord_angle * (index + 0.5)
                    T.rotation(0.0, angle, 0.0) {
                        T.position(rail_radius * Math.cos(chord_angle / 2.0), -rail_height / 2.0, 0.0)
                            .scale(rail_thickness, rail_height, length) {
                            R.cube() {
                                C.rgba(rail_color[0], rail_color[1], rail_color[2], 1.0)
                            }
                        }
                    }
                }
            }
        }
    }
}

// A continuous deck just below the sleepers with an exposed walking strip on
// the outside of the curve. Its top stays below the cars' sweep sensors.
export fn railroad_walkway(radius, start, end, base_height, material) {
    let count = Math.floor(Math.abs(end - start) * radius / 1.5) + 1
    let step = (end - start) / count
    let length = 2.0 * (radius + 2.1) * Math.sin(Math.abs(step) / 2.0) + 0.02
    return T.position(0.0, base_height - 0.32, 0.0) {
        for index in range(count) {
            T.rotation(0.0, start + (index + 0.5) * step, 0.0) {
                T.position((radius + 0.5) * Math.cos(step / 2.0), 0.0, 0.0).scale(3.2, 0.12, length) {
                    R.cube() { material C.rgba(0.33, 0.30, 0.29, 1.0) }
                    Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
                }
            }
        }
    }
}
