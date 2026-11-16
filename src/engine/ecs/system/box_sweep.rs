//! Continuous oriented-box contact for translating boxes against static boxes.
//!
//! Both boxes are convex polyhedra with face normals on their own axes, so
//! the 15 separating-axis candidates (3 + 3 face axes, 9 edge cross products)
//! are exact for pure translation. Rotation during one fixed step is not
//! swept: callers sample the mover's orientation at the start of the step.
use crate::engine::ecs::component::CollisionShape;

/// World-space center, half extents, and orthonormal axes.
pub(crate) type OrientedBox = ([f32; 3], [f32; 3], [[f32; 3]; 3]);

const AXIS_EPSILON: f32 = 1.0e-6;
/// Penetration below this depth is touching, not contact. A box flush against
/// a floor or sliding through an opening with zero clearance stays clear.
pub(crate) const CONTACT_SKIN: f32 = 1.0e-4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BoxSweepHit {
    pub fraction: f32,
    pub point: [f32; 3],
    pub normal: [f32; 3],
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// A cube zone's world box under any rotation. Shear is rejected rather than
/// approximated.
pub(crate) fn oriented_box(shape: CollisionShape, frame: [[f32; 4]; 4]) -> Option<OrientedBox> {
    let CollisionShape::Cube { half_extents } = shape else {
        return None;
    };
    let lengths: [f32; 3] = std::array::from_fn(|i| {
        (frame[i][0].powi(2) + frame[i][1].powi(2) + frame[i][2].powi(2)).sqrt()
    });
    if lengths.iter().any(|v| !v.is_finite() || *v <= AXIS_EPSILON) {
        return None;
    }
    let axes: [[f32; 3]; 3] =
        std::array::from_fn(|i| std::array::from_fn(|j| frame[i][j] / lengths[i]));
    if (0..3).any(|i| (0..i).any(|j| dot(axes[i], axes[j]).abs() > 1.0e-4)) {
        return None;
    }
    Some((
        [frame[3][0], frame[3][1], frame[3][2]],
        std::array::from_fn(|i| half_extents[i] * lengths[i]),
        axes,
    ))
}

/// World AABB of a box swept by `delta`.
pub(crate) fn swept_bounds(moving: &OrientedBox, delta: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let (center, extents, axes) = moving;
    let reach: [f32; 3] =
        std::array::from_fn(|i| (0..3).map(|a| axes[a][i].abs() * extents[a]).sum());
    (
        std::array::from_fn(|i| center[i].min(center[i] + delta[i]) - reach[i]),
        std::array::from_fn(|i| center[i].max(center[i] + delta[i]) + reach[i]),
    )
}

/// Earliest contact in `[0, 1]` of `moving` translated by `delta` against
/// `fixed`. Initial overlap reports fraction 0 with the shallowest axis.
pub(crate) fn sweep(
    moving: &OrientedBox,
    delta: [f32; 3],
    fixed: &OrientedBox,
) -> Option<BoxSweepHit> {
    let (a_center, a_extents, a_axes) = moving;
    let (b_center, b_extents, b_axes) = fixed;
    let offset: [f32; 3] = std::array::from_fn(|i| a_center[i] - b_center[i]);
    let mut enter = f32::NEG_INFINITY;
    let mut exit = f32::INFINITY;
    let mut enter_normal = [0.0; 3];
    let mut shallowest = (f32::INFINITY, [0.0; 3]);
    let mut candidates = [[0.0f32; 3]; 15];
    candidates[..3].copy_from_slice(a_axes);
    candidates[3..6].copy_from_slice(b_axes);
    for i in 0..3 {
        for j in 0..3 {
            candidates[6 + i * 3 + j] = cross(a_axes[i], b_axes[j]);
        }
    }
    for axis in candidates {
        let length = dot(axis, axis).sqrt();
        if length <= AXIS_EPSILON {
            continue;
        }
        let axis = axis.map(|v| v / length);
        let radius: f32 = (0..3)
            .map(|k| a_extents[k] * dot(a_axes[k], axis).abs() + b_extents[k] * dot(b_axes[k], axis).abs())
            .sum::<f32>()
            - CONTACT_SKIN;
        let separation = dot(offset, axis);
        let speed = dot(delta, axis);
        let depth = radius - separation.abs();
        if depth < shallowest.0 {
            let sign = if separation < 0.0 { -1.0 } else { 1.0 };
            shallowest = (depth, axis.map(|v| v * sign));
        }
        if speed.abs() <= AXIS_EPSILON {
            if separation.abs() >= radius {
                return None;
            }
            continue;
        }
        let (t0, t1) = ((-radius - separation) / speed, (radius - separation) / speed);
        let (near, far) = if t0 < t1 { (t0, t1) } else { (t1, t0) };
        if near > enter {
            enter = near;
            // Facing the incoming mover: opposite the relative motion.
            let sign = if speed > 0.0 { -1.0 } else { 1.0 };
            enter_normal = axis.map(|v| v * sign);
        }
        exit = exit.min(far);
        if enter > exit || enter > 1.0 || exit < 0.0 {
            return None;
        }
    }
    let (fraction, normal) = if enter <= 0.0 {
        (0.0, shallowest.1)
    } else {
        (enter, enter_normal)
    };
    // Report the point on the fixed box nearest the mover's center at contact.
    let center: [f32; 3] = std::array::from_fn(|i| a_center[i] + delta[i] * fraction);
    let relative: [f32; 3] = std::array::from_fn(|i| center[i] - b_center[i]);
    let mut point = *b_center;
    for k in 0..3 {
        let d = dot(relative, b_axes[k]).clamp(-b_extents[k], b_extents[k]);
        for i in 0..3 {
            point[i] += b_axes[k][i] * d;
        }
    }
    Some(BoxSweepHit {
        fraction,
        point,
        normal,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    fn rolled(angle: f32) -> [[f32; 3]; 3] {
        let (s, c) = angle.sin_cos();
        [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]]
    }

    /// Wall in the z = `z` plane with a square opening of side `side`.
    fn square_aperture(z: f32, side: f32) -> Vec<OrientedBox> {
        let h = side / 2.0;
        vec![
            ([-(h + 2.0), 0.0, z], [2.0, h + 4.0, 0.1], IDENTITY),
            ([h + 2.0, 0.0, z], [2.0, h + 4.0, 0.1], IDENTITY),
            ([0.0, h + 2.0, z], [h, 2.0, 0.1], IDENTITY),
            ([0.0, -(h + 2.0), z], [h, 2.0, 0.1], IDENTITY),
        ]
    }

    /// A 32-gon of rolled boxes whose inner faces sit at `radius`.
    fn round_aperture(z: f32, radius: f32) -> Vec<OrientedBox> {
        (0..32)
            .map(|k| {
                let angle = k as f32 * std::f32::consts::TAU / 32.0;
                let mid = radius + 0.5;
                (
                    [mid * angle.cos(), mid * angle.sin(), z],
                    [0.5, (radius + 1.0) * (std::f32::consts::PI / 32.0).tan() + 0.01, 0.1],
                    rolled(angle),
                )
            })
            .collect()
    }

    fn first_hit(moving: &OrientedBox, delta: [f32; 3], walls: &[OrientedBox]) -> Option<BoxSweepHit> {
        walls
            .iter()
            .filter_map(|wall| sweep(moving, delta, wall))
            .min_by(|a, b| a.fraction.total_cmp(&b.fraction))
    }

    // Cross-section 1.94 x 2.2 (the train sensor). Its 2.93 diagonal exceeds
    // a 2.7 circle while both sides fit a 2.7 square.
    fn train(z: f32) -> OrientedBox {
        ([0.0, 0.0, z], [0.97, 1.1, 2.06], IDENTITY)
    }

    #[test]
    fn same_size_square_passes_where_round_blocks_the_box() {
        let delta = [0.0, 0.0, -20.0];
        assert_eq!(first_hit(&train(10.0), delta, &square_aperture(0.0, 2.7)), None);
        let hit = first_hit(&train(10.0), delta, &round_aperture(0.0, 1.35)).unwrap();
        // Front face (z = 7.94) reaches the ring's near face at z = 0.1.
        assert!((hit.fraction - 7.84 / 20.0).abs() < 1e-4, "{hit:?}");
        assert!(hit.normal[2] > 0.99, "{hit:?}");
        assert!(hit.point.iter().all(|v| v.is_finite()));
        // Control: a sphere-sized box (1.1 half extents) inscribed in the
        // same cross-section does fit the circle.
        let inscribed = ([0.0, 0.0, 10.0], [0.95, 0.95, 2.06], IDENTITY);
        assert_eq!(first_hit(&inscribed, delta, &round_aperture(0.0, 1.35)), None);
    }

    #[test]
    fn clearance_around_the_limiting_dimension() {
        let delta = [0.0, 0.0, -20.0];
        // 2.2 tall: a 2.21 square clears, a 2.19 square blocks.
        assert_eq!(first_hit(&train(10.0), delta, &square_aperture(0.0, 2.21)), None);
        assert!(first_hit(&train(10.0), delta, &square_aperture(0.0, 2.19)).is_some());
        // Exactly flush is touching, not contact.
        assert_eq!(first_hit(&train(10.0), delta, &square_aperture(0.0, 2.2)), None);
    }

    #[test]
    fn fast_crossing_initial_overlap_and_miss() {
        let thin = ([0.0, 0.0, 0.0], [5.0, 5.0, 0.01], IDENTITY);
        // Both endpoints are clear of a 2 cm wall; the sweep still hits it.
        let hit = sweep(&train(10.0), [0.0, 0.0, -30.0], &thin).unwrap();
        assert!(hit.fraction > 0.0 && hit.fraction < 1.0);
        let overlapping = sweep(&train(1.0), [0.0, 0.0, -1.0], &thin).unwrap();
        assert_eq!(overlapping.fraction, 0.0);
        assert!(sweep(&train(10.0), [8.0, 0.0, 0.0], &thin).is_none());
        assert!(sweep(&train(10.0), [0.0, 0.0, -1.0], &thin).is_none());
    }

    #[test]
    fn yawed_mover_uses_edge_axes() {
        // A 45-degree yawed unit cube approaching a box corner-first reaches
        // it later than its axis-aligned bounds suggest.
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let yawed = ([0.0, 0.0, 5.0], [0.5; 3], [[s, 0.0, -s], [0.0, 1.0, 0.0], [s, 0.0, s]]);
        let wall = ([0.0, 0.0, 0.0], [5.0, 5.0, 0.5], IDENTITY);
        let hit = sweep(&yawed, [0.0, 0.0, -5.0], &wall).unwrap();
        let corner = 0.5 * std::f32::consts::SQRT_2;
        assert!((hit.fraction - (5.0 - 0.5 - corner) / 5.0).abs() < 1e-4, "{hit:?}");
    }

    #[test]
    fn oriented_box_accepts_rotation_and_rejects_shear() {
        let (s, c) = 0.3f32.sin_cos();
        let rolled_frame = [[2.0 * c, 2.0 * s, 0.0, 0.0], [-s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [1.0, 2.0, 3.0, 1.0]];
        let (center, extents, _) = oriented_box(CollisionShape::CUBE(), rolled_frame).unwrap();
        assert_eq!(center, [1.0, 2.0, 3.0]);
        assert!((extents[0] - 1.0).abs() < 1e-6);
        let sheared = [[1.0, 0.0, 0.0, 0.0], [0.5, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
        assert!(oriented_box(CollisionShape::CUBE(), sheared).is_none());
        assert!(oriented_box(CollisionShape::SPHERE(), rolled_frame).is_none());
    }
}
