fn terrain_height(cell_x, cell_z) {
    let broad = Math.perlin(cell_x * 0.18 + 17.0, 0.0, cell_z * 0.18 - 9.0)
    let detail = Math.perlin(cell_x * 0.43 - 31.0, 3.0, cell_z * 0.43 + 11.0)
    let blended = broad * 0.8 + detail * 0.48

    return 1.0 + Math.floor((blended + 1.0) * 1.5)
}


fn terrain_cube(x, y, z, color) {
    return T.position(x, y, z).scale(3.0, 3.0, 3.0) {
        Collision.static() {
            CollisionShape.cube([3, 3, 3])
        }
        R.cube() {
            C.rgba(color[0], color[1], color[2], color[3])
        }
    }
}

export fn voxel_terrain(config) {
    let grid_size = 72.0
    let cube_size = 3.0
    let cube_half = cube_size * 0.5
    let grid_half = grid_size * 0.5
    let base_y = -3.15

    

    return Raycastable.enabled() {
        T {
            for z in range(grid_size) {
                for x in range(grid_size) {
                    let height_steps = terrain_height(x, z)
                    let snapped_x = (x - grid_half) * cube_size
                    let snapped_z = (z - grid_half) * cube_size
                    let level = height_steps - 1.0

                    let color = [1,1,1,1]

                    let snapped_y = base_y + level * cube_size
                    terrain_cube(
                        snapped_x + cube_half,
                        snapped_y + cube_half,
                        snapped_z + cube_half,
                        color,
                    )
                }
            }
        }
    }
}
