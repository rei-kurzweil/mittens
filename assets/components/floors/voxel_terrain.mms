fn terrain_level(cell_x, cell_z) {
    let broad = Math.perlin(cell_x * 0.18 + 17.0, 0.0, cell_z * 0.18 - 9.0)
    let detail = Math.perlin(cell_x * 0.43 - 31.0, 3.0, cell_z * 0.43 + 11.0)
    // Let the broad field form connected shelves. Detail can soften their
    // edges without overwhelming the large shapes.
    let shaped = broad * 0.85 + detail * 0.20

    if shaped < -0.28 { return 0.0 }
    if shaped < -0.10 { return 1.0 }
    if shaped < 0.08 { return 2.0 }
    return 3.0
}

fn grass_surface_offset(cell_x, cell_z) {
    let patch = Math.perlin(cell_x * 0.16 + 43.0, 7.0, cell_z * 0.16 - 27.0)
    return Math.clamp(patch * 0.12, -0.05, 0.05)
}

fn terrain_cube(x, y, z, color) {
    return T.position(x, y, z).scale(3.0, 3.0, 3.0) {
        // Zone dimensions are local: the transform scales this unit cube.
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static() {} }
        R.cube() {
            C.rgba(color[0], color[1], color[2], color[3])
        }
    }
}

fn terrain_color(palette, y, base_y, cube_size) {
    if y <= base_y {
        return palette[0]
    }
    if y <= base_y + cube_size {
        return palette[1]
    }
    if y <= base_y + cube_size * 2.0 {
        return palette[2]
    }
    return palette[3]
}

// All config fields are optional. Palette order is low-to-high:
// [water, rock, dirt, grass].
export fn voxel_terrain(config) {
    let grid_length = 72.0
    let grid_width = 72.0
    // Low to high: water, rock, dirt, grass.
    let palette = [
        [0.30, 0.70, 0.90, 1.0],
        [0.30, 0.32, 0.38, 1.0],
        [0.42, 0.23, 0.12, 1.0],
        [0.42, 0.80, 0.36, 1.0],
    ]
    if config {
        let configured_length = config["length"]
        let configured_width = config["width"]
        let configured_palette = config["palette"]
        if configured_length { grid_length = configured_length }
        if configured_width { grid_width = configured_width }
        if configured_palette { palette = configured_palette }
    }
    let cube_size = 3.0
    let cube_half = cube_size * 0.5
    // The terrain is centered approximately around its prefab origin, but its
    // X/Z cell boundaries are authoritative: every cube's back-left corner
    // must land on whole prefab-local units. Flooring the half extent keeps
    // that invariant for odd as well as even terrain dimensions.
    let terrain_origin_x = 0.0 - Math.floor((grid_width * cube_size) * 0.5)
    let terrain_origin_z = 0.0 - Math.floor((grid_length * cube_size) * 0.5)
    let base_y = -3.15

    return Raycastable.enabled() {
        T {
            for z in range(grid_length) {
                for x in range(grid_width) {
                    let level = terrain_level(x, z)
                    let cell_min_x = terrain_origin_x + x * cube_size
                    let cell_min_z = terrain_origin_z + z * cube_size

                    let snapped_y = base_y + level * cube_size
                    let color = terrain_color(palette, snapped_y, base_y, cube_size)
                    let surface_y = snapped_y
                    if level == 3.0 {
                        surface_y = surface_y + grass_surface_offset(x, z)
                    }
                    terrain_cube(
                        cell_min_x + cube_half,
                        surface_y + cube_half,
                        cell_min_z + cube_half,
                        color,
                    )
                }
            }
        }
    }
}
