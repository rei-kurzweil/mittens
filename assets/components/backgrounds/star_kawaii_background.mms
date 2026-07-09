fn hash01(seed) {
    let x = Math.sin(seed * 12.9898 + 78.233) * 43758.5453
    return x - Math.floor(x)
}

fn star_scale(seed) {
    return 0.22 + hash01(seed + 8.0) * 0.55
}


fn star_instance(index, radius, color) {
    let seed = index + 1.0

    let orbit_yaw = hash01(seed + 3.0) * 6.28318530717959
    let orbit_pitch = (hash01(seed + 5.0) - 0.5) * 1.25

    let cos_pitch = Math.cos(orbit_pitch)
    let sin_pitch = Math.sin(orbit_pitch)
    let cos_yaw = Math.cos(orbit_yaw)
    let sin_yaw = Math.sin(orbit_yaw)

    let x = radius * cos_pitch * cos_yaw
    let y = radius * sin_pitch
    let z = radius * cos_pitch * sin_yaw

    let scale = star_scale(seed) * 8.0

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
        for i in range(300) {
            star_instance(i, radius, color)
        }
    }
}
