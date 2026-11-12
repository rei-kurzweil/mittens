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
