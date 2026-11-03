// Low rolling mounds grown from smoothly fused sphere fields, clipped into the
// upper deck slab so no hidden sphere geometry protrudes into the washrooms.
fn smooth_min(a, b) {
    let h = Math.clamp(0.5 + 0.5 * (b - a) / 0.7, 0.0, 1.0)
    return b * (1.0 - h) + a * h - 0.7 * h * (1.0 - h)
}

export fn park_ground_height(x, z) {
    let low = 0.0
    let high = 0.8
    for iteration in range(18) {
        let y = (low + high) / 2.0
        let field = 100.0
        for cx in [-3.8, 3.8] {
            for cz in [-2.8, 2.8] {
                let dx = x - cx
                let dz = z - cz
                let a = Math.sqrt(dx*dx + (y+2.05)*(y+2.05) + dz*dz) - 2.4
                let b = Math.sqrt((dx-0.65)*(dx-0.65) + (y+2.1)*(y+2.1) + (dz-0.2)*(dz-0.2)) - 2.3
                field = smooth_min(field, a)
                field = smooth_min(field, b)
            }
        }
        if field < 0.0 { low = y } else { high = y }
    }
    return high
}

export fn ship_park_ground() {
    let grass = Material(MaterialDefinition.anime())
    return T {
        name = "ship_park_ground"
        T.position(0.0, 0.004, 0.0).rotation(-Math.pi/2.0, 0.0, 0.0).scale(16.0, 12.0, 1.0) {
            R.plane() {
                grass C.rgba(1.0, 1.0, 1.0, 1.0)
                UV.uv(-8.0, 6.0).uv(8.0, 6.0).uv(8.0, -6.0).uv(-8.0, -6.0)
                Texture.with_uri("assets/textures/grass/ship-park-grass.png")
                TextureFiltering.linear()
            }
        }
        ImplicitSurface.bounds(-6.9, -0.30, -5.5, 6.9, 1.0, 5.5)
            .voxel_size(0.12).smooth_min_radius(0.7).clip_floor(-0.08) {
            name = "ship_park_rolling_hills"
            grass C.rgba(1.0, 1.0, 1.0, 1.0)
            Texture.with_uri("assets/textures/grass/ship-park-grass.png")
            TextureFiltering.linear()
            // This zone receives the generated triangle mesh, keeping foot
            // contacts on the same smooth surface that is rendered.
            Zone.cube([6.9, 1.0, 5.5]) { Collidable.static().friction(0.8) {} }
            for x in [-3.8, 3.8] {
                for z in [-2.8, 2.8] {
                    T.position(x, -2.05, z) { ImplicitSphere.radius(2.4) }
                    T.position(x+0.65, -2.10, z+0.2) { ImplicitSphere.radius(2.3) }
                }
            }
        }
    }
}
