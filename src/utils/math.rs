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

pub fn quat_from_axis_angle(axis: [f32; 3], angle_rad: f32) -> [f32; 4] {
    let axis = vec3_normalize(axis);
    let (s, c) = (0.5 * angle_rad).sin_cos();
    [axis[0] * s, axis[1] * s, axis[2] * s, c]
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
