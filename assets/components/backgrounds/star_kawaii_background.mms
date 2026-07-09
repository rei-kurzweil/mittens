fn hash01(seed) {
    let x = Math.sin(seed * 12.9898 + 78.233) * 43758.5453
    return x - Math.floor(x)
}

fn star_scale(seed) {
    return 0.22 + hash01(seed + 8.0) * 0.55
}

fn star_twist(seed) {
    return (hash01(seed + 23.0) - 0.5) * 0.7
}

fn star_color(seed) {
    let t = hash01(seed + 41.0)

    if t < 0.2 { return [1.0, 0.82, 0.96, 1.0] }
    if t < 0.4 { return [0.86, 0.93, 1.0, 1.0] }
    if t < 0.6 { return [1.0, 0.97, 0.76, 1.0] }
    if t < 0.8 { return [0.82, 1.0, 0.92, 1.0] }

    return [0.96, 0.84, 1.0, 1.0]
}

fn quat_mul(a, b) {
    return [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
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
    let twist = star_twist(seed) / 2.0
    let dir_x = -x / radius
    let dir_y = -y / radius
    let dir_z = -z / radius

    let look_raw = [-dir_y, dir_x, 0.0, 1.0 + dir_z]
    let look_inv_len = 1.0 / Math.sqrt(
        look_raw[0] * look_raw[0] +
        look_raw[1] * look_raw[1] +
        look_raw[2] * look_raw[2] +
        look_raw[3] * look_raw[3]
    )
    let look = [
        look_raw[0] * look_inv_len,
        look_raw[1] * look_inv_len,
        look_raw[2] * look_inv_len,
        look_raw[3] * look_inv_len,
    ]

    let half_twist = twist / 2.0
    let twist_quat = [0.0, 0.0, Math.sin(half_twist), Math.cos(half_twist)]
    let rotation = quat_mul(look, twist_quat)

    return T
        .position(x, y, z)
        .quat(rotation)
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
