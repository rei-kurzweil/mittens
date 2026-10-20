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

fn star_instance(dir, radius, color, size_scale) {
    let radial_noise = Math.perlin(dir[0] * 2.4, dir[1] * 2.4, dir[2] * 2.4)
    let r = radius + radial_noise * 6.0
    let x = dir[0] * r
    let y = dir[1] * r
    let z = dir[2] * r

    let scale = (0.2 + radial_noise * 4.0) * size_scale

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

// Optional count is the spherical sample budget before density filtering
// (default 320); scale multiplies each star's existing varied size (default 1).
// Example: star_kawaii_background(color, { count = 160 scale = 2.0 })
export fn star_kawaii_background(color, options) {
    let radius = 48.0
    let count = 320
    let size_scale = 1.0
    if options != null {
        if options["count"] != null { count = Math.floor(options.count) }
        if options["scale"] != null { size_scale = options.scale }
    }
    if count < 0 { count = 0 }
    if size_scale < 0.0 { size_scale = 0.0 }

    return T {
        for i in range(count) {
            let dir = sphere_direction(i, count)
            let density = Math.perlin(
                dir[0] * 4.2 + 13.7,
                dir[1] * 4.2 - 2.1,
                dir[2] * 4.2 + 7.9,
            )
            if density > -0.08 {
                star_instance(dir, radius, color, size_scale)
            }
        }
    }
}
