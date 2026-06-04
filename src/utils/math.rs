pub fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let (ax, ay, az, aw) = (a[0], a[1], a[2], a[3]);
    let (bx, by, bz, bw) = (b[0], b[1], b[2], b[3]);
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

pub fn quat_conjugate(q: [f32; 4]) -> [f32; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

pub fn quat_rotate_vec3(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    // v' = q * (v,0) * conj(q)
    let vq = [v[0], v[1], v[2], 0.0f32];
    let t = quat_mul(q, vq);
    let r = quat_mul(t, quat_conjugate(q));
    [r[0], r[1], r[2]]
}

pub fn vec3_normalize(v: [f32; 3]) -> [f32; 3] {
    let len2 = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if len2 <= 0.0 {
        return [0.0, 0.0, 0.0];
    }
    let inv = len2.sqrt().recip();
    [v[0] * inv, v[1] * inv, v[2] * inv]
}

pub fn vec3_len(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

pub fn vec3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn vec3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn vec3_scale(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

pub fn vec3_sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn mat4_identity() -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub fn mat4_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0f32; 4]; 4];
    for c in 0..4 {
        for r in 0..4 {
            out[c][r] =
                a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2] + a[3][r] * b[c][3];
        }
    }
    out
}

pub fn vec3_add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn vec3_lerp(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    vec3_add(a, vec3_scale(vec3_sub(b, a), t))
}

pub fn quat_normalize(q: [f32; 4]) -> [f32; 4] {
    let len2 = q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3];
    if len2 < 1e-12 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let inv = len2.sqrt().recip();
    [q[0] * inv, q[1] * inv, q[2] * inv, q[3] * inv]
}

pub fn quat_rotation_y(yaw: f32) -> [f32; 4] {
    let half = yaw * 0.5;
    [0.0, half.sin(), 0.0, half.cos()]
}

/// Normalised linear interpolation between two unit quaternions.
/// Ensures shortest-path by negating `b` if the dot product is negative.
pub fn quat_nlerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    let b = if dot < 0.0 {
        [-b[0], -b[1], -b[2], -b[3]]
    } else {
        b
    };
    quat_normalize([
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ])
}

/// Minimum-arc quaternion rotating unit vector `from` to unit vector `to`.
pub fn shortest_arc_quat(from: [f32; 3], to: [f32; 3]) -> [f32; 4] {
    let d = vec3_dot(from, to);
    if d < -0.9999 {
        let perp = if from[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let axis = vec3_normalize(vec3_cross(from, perp));
        return [axis[0], axis[1], axis[2], 0.0];
    }
    let c = vec3_cross(from, to);
    quat_normalize([c[0], c[1], c[2], 1.0 + d])
}

/// Extract a unit quaternion from a column-major 4x4 world matrix (may have scale).
pub fn mat_to_quat(m: [[f32; 4]; 4]) -> [f32; 4] {
    fn col_len(m: [[f32; 4]; 4], c: usize) -> f32 {
        (m[c][0] * m[c][0] + m[c][1] * m[c][1] + m[c][2] * m[c][2])
            .sqrt()
            .max(1e-9)
    }
    let s0 = col_len(m, 0).recip();
    let s1 = col_len(m, 1).recip();
    let s2 = col_len(m, 2).recip();
    let r00 = m[0][0] * s0;
    let r10 = m[0][1] * s0;
    let r20 = m[0][2] * s0;
    let r01 = m[1][0] * s1;
    let r11 = m[1][1] * s1;
    let r21 = m[1][2] * s1;
    let r02 = m[2][0] * s2;
    let r12 = m[2][1] * s2;
    let r22 = m[2][2] * s2;
    let trace = r00 + r11 + r22;
    if trace > 0.0 {
        let s = 0.5 / (trace + 1.0).sqrt();
        quat_normalize([(r21 - r12) * s, (r02 - r20) * s, (r10 - r01) * s, 0.25 / s])
    } else if r00 > r11 && r00 > r22 {
        let s = 2.0 * (1.0 + r00 - r11 - r22).sqrt();
        quat_normalize([0.25 * s, (r01 + r10) / s, (r02 + r20) / s, (r21 - r12) / s])
    } else if r11 > r22 {
        let s = 2.0 * (1.0 + r11 - r00 - r22).sqrt();
        quat_normalize([(r01 + r10) / s, 0.25 * s, (r12 + r21) / s, (r02 - r20) / s])
    } else {
        let s = 2.0 * (1.0 + r22 - r00 - r11).sqrt();
        quat_normalize([(r02 + r20) / s, (r12 + r21) / s, 0.25 * s, (r10 - r01) / s])
    }
}

pub fn quat_from_axis_angle(axis: [f32; 3], angle_rad: f32) -> [f32; 4] {
    let axis = vec3_normalize(axis);
    let (s, c) = (0.5 * angle_rad).sin_cos();
    [axis[0] * s, axis[1] * s, axis[2] * s, c]
}

/// Extract rotation axis and angle (radians) from a quaternion.
/// Returns (axis, angle) where angle is in range [-π, π].
/// For near-zero rotations, returns ([0, 0, 1], 0).
pub fn quat_to_axis_angle(q: [f32; 4]) -> ([f32; 3], f32) {
    let [x, y, z, w] = q;
    let len_sq = x * x + y * y + z * z;

    if len_sq < 1e-10 {
        // Near identity quaternion
        return ([0.0, 0.0, 1.0], 0.0);
    }

    let sin_half_angle = len_sq.sqrt();
    let angle = 2.0 * sin_half_angle.atan2(w);
    let axis = if sin_half_angle > 1e-6 {
        [x / sin_half_angle, y / sin_half_angle, z / sin_half_angle]
    } else {
        [0.0, 0.0, 1.0]
    };

    (axis, angle)
}

/// Invert a column-major 4x4 matrix.
///
/// Returns `None` if the matrix is singular.
pub fn mat4_inverse(m: [[f32; 4]; 4]) -> Option<[[f32; 4]; 4]> {
    // Generic 4x4 inverse via Gauss-Jordan elimination on an augmented matrix.
    // Treat input as row-major for elimination convenience by transposing access.
    let mut a = [[0.0f32; 8]; 4];
    for r in 0..4 {
        for c in 0..4 {
            // Convert column-major m[c][r] into row-major a[r][c].
            a[r][c] = m[c][r];
        }
        a[r][4 + r] = 1.0;
    }

    for i in 0..4 {
        // Find pivot.
        let mut pivot_row = i;
        let mut pivot_val = a[i][i].abs();
        for r in (i + 1)..4 {
            let v = a[r][i].abs();
            if v > pivot_val {
                pivot_val = v;
                pivot_row = r;
            }
        }
        if pivot_val == 0.0 {
            return None;
        }
        if pivot_row != i {
            a.swap(i, pivot_row);
        }

        // Normalize pivot row.
        let inv_pivot = 1.0 / a[i][i];
        for c in i..8 {
            a[i][c] *= inv_pivot;
        }

        // Eliminate other rows.
        for r in 0..4 {
            if r == i {
                continue;
            }
            let factor = a[r][i];
            if factor == 0.0 {
                continue;
            }
            for c in i..8 {
                a[r][c] -= factor * a[i][c];
            }
        }
    }

    // Extract inverse (row-major) and convert back to column-major.
    let mut inv = [[0.0f32; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            inv[c][r] = a[r][4 + c];
        }
    }
    Some(inv)
}
