import { ship_environment_material } from "../materials/ship_anime_materials.mms"
// Rolling mounds grown from smoothly fused sphere fields, clipped into the
// upper deck slab so no hidden sphere geometry protrudes into the washrooms.
fn ground_profile(extent_factor) {
    if extent_factor == 4 {
        return { xs=[-24,-8,8,24] zs=[-16,-6,6,16] radius_a=4.4 radius_b=4.2 height_limit=3.0 }
    }
    return { xs=[-3.8,3.8] zs=[-2.8,2.8] radius_a=2.4 radius_b=2.3 height_limit=1.0 }
}

fn smooth_min(a, b) {
    let h = Math.clamp(0.5 + 0.5 * (b - a) / 0.7, 0.0, 1.0)
    return b * (1.0 - h) + a * h - 0.7 * h * (1.0 - h)
}

export fn park_ground_height(x, z, extent_factor) {
    let profile = ground_profile(extent_factor)
    let low = 0.0
    let high = profile.height_limit
    for iteration in range(18) {
        let y = (low + high) / 2.0
        let field = 100.0
        for cx in profile.xs {
            for cz in profile.zs {
                let dx = x - cx
                let dz = z - cz
                let a = Math.sqrt(dx*dx + (y+2.05)*(y+2.05) + dz*dz) - profile.radius_a
                let b = Math.sqrt((dx-0.65)*(dx-0.65) + (y+2.1)*(y+2.1) + (dz-0.2)*(dz-0.2)) - profile.radius_b
                field = smooth_min(field, a)
                field = smooth_min(field, b)
            }
        }
        if field < 0.0 { low = y } else { high = y }
    }
    return high
}

export fn ship_park_ground(extent_factor) {
    if extent_factor == null { extent_factor=1 }
    let profile = ground_profile(extent_factor)
    let voxel = 0.12
    if extent_factor == 4 { voxel=0.24 }
    let grass = ship_environment_material()
    return T {
        name = "ship_park_ground"
        T.position(0.0, 0.004, 0.0).rotation(-Math.pi/2.0, 0.0, 0.0).scale(16.0*extent_factor,12.0*extent_factor,1.0) {
            R.plane() {
                grass C.rgba(1.0, 1.0, 1.0, 1.0)
                UV.uv(-8*extent_factor,6*extent_factor).uv(8*extent_factor,6*extent_factor).uv(8*extent_factor,-6*extent_factor).uv(-8*extent_factor,-6*extent_factor)
                Texture.with_uri("assets/textures/grass/ship-park-grass.png")
                TextureFiltering.linear()
            }
        }
        let chunks = 1
        if extent_factor == 4 { chunks = 4 }
        for chunk in range(chunks) {
            let xlo=-6.9 let xhi=6.9 let zlo=-5.5 let zhi=5.5
            let label="ship_park_rolling_hills"
            if extent_factor == 4 {
                xlo=-29 xhi=0 zlo=-22 zhi=0
                if chunk%2 == 1 { xlo=0 xhi=29 }
                if chunk>=2 { zlo=0 zhi=22 }
                if chunk>0 { label=label+"_"+chunk }
            }
        ImplicitSurface.bounds(xlo,-0.30,zlo,xhi,profile.height_limit,zhi)
            .voxel_size(voxel).smooth_min_radius(0.7).clip_floor(-0.08) {
            name = label
            grass C.rgba(1.0, 1.0, 1.0, 1.0)
            Texture.with_uri("assets/textures/grass/ship-park-grass.png")
            TextureFiltering.linear()
            // This zone receives the generated triangle mesh, keeping foot
            // contacts on the same smooth surface that is rendered.
            Zone.cube([7.25*extent_factor,profile.height_limit,5.5*extent_factor]) { Collidable.static().friction(0.8) {} }
            for x in profile.xs {
                for z in profile.zs {
                    T.position(x, -2.05, z) { ImplicitSphere.radius(profile.radius_a) }
                    T.position(x+0.65, -2.10, z+0.2) { ImplicitSphere.radius(profile.radius_b) }
                }
            }
        }
        }
    }
}
