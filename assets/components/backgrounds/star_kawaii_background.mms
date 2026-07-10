fn hash01(seed) {
    let x = Math.sin(seed * 12.9898 + 78.233) * 43758.5453
    return x - Math.floor(x)
}

// fn star_scale(seed) {
//     return 0.22 + hash01(seed + 8.0) * 0.55
// }

fn sphere_direction(index, count) {
    let i = index + 0.5
    let y = 1.0 - (2.0 * i / count)
    let ring_radius = Math.sqrt(1.0 - y * y)
    let theta = i * 2.399963229728653

    return [
        Math.cos(theta) * ring_radius,
        y,
        Math.sin(theta) * ring_radius,
    ]
}

fn star_instance(index, radius, color) {
    let seed = index + 1.0
    let dir = sphere_direction(index, 320.0)
    let radial_noise = Math.perlin(dir[0] * 2.4, dir[1] * 2.4, dir[2] * 2.4)
    let r = radius + radial_noise * 6.0
    let x = dir[0] * r
    let y = dir[1] * r
    let z = dir[2] * r

    let scale = 0.2 + radial_noise * 4.0

    return T
        .position(x, y, z)
        .looking_at([0, 0, 0])
        .scale(scale, scale, scale) {
        R.star(5, 0.48, 10, 10) {
            C.rgba(color[0], color[1], color[2], color[3])
            EM.on() {
                intensity(1.9)
            }
        }
    }
}

export fn star_kawaii_background(color) {
    let radius = 48.0

    return T {
        for i in range(320) {
            let dir = sphere_direction(i, 320.0)
            let density = Math.perlin(
                dir[0] * 4.2 + 13.7,
                dir[1] * 4.2 - 2.1,
                dir[2] * 4.2 + 7.9,
            )
            if density > -0.08 {
                star_instance(i, radius, color)
            }
        }
    }
}
