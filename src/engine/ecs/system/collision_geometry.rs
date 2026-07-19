//! Shared, axis-aligned collision geometry used by detection and response.

use crate::engine::ecs::component::CollisionShape;

const GEOMETRY_EPSILON: f32 = 1.0e-6;

pub(crate) fn world_aabb(center: [f32; 3], shape: CollisionShape) -> ([f32; 3], [f32; 3]) {
    let (min, max) = shape.normalized().aabb_local();
    (
        [center[0] + min[0], center[1] + min[1], center[2] + min[2]],
        [center[0] + max[0], center[1] + max[1], center[2] + max[2]],
    )
}

/// Inclusive contact test. Exact tangency counts as contact for collision events.
pub(crate) fn intersects(
    a_center: [f32; 3],
    a_shape: CollisionShape,
    b_center: [f32; 3],
    b_shape: CollisionShape,
) -> bool {
    penetration_vector(a_center, a_shape, b_center, b_shape, 0.0, true).is_some()
}

/// Minimum translation that moves A out of B. Exact tangency produces no response.
pub(crate) fn minimum_translation(
    a_center: [f32; 3],
    a_shape: CollisionShape,
    b_center: [f32; 3],
    b_shape: CollisionShape,
    push_out_epsilon: f32,
) -> Option<[f32; 3]> {
    penetration_vector(
        a_center,
        a_shape,
        b_center,
        b_shape,
        push_out_epsilon.max(0.0),
        false,
    )
}

fn penetration_vector(
    a_center: [f32; 3],
    a_shape: CollisionShape,
    b_center: [f32; 3],
    b_shape: CollisionShape,
    extra: f32,
    inclusive: bool,
) -> Option<[f32; 3]> {
    use CollisionShape::*;
    let a_shape = a_shape.normalized();
    let b_shape = b_shape.normalized();
    match (a_shape, b_shape) {
        (Cube { half_extents: a }, Cube { half_extents: b }) => {
            box_box_mtv(a_center, a, b_center, b, extra, inclusive)
        }
        (Sphere { radius: a }, Sphere { radius: b }) => {
            sphere_sphere_mtv(a_center, a, b_center, b, extra, inclusive)
        }
        (
            CapsuleY {
                radius,
                half_segment,
            },
            Sphere { radius: b },
        ) => capsule_sphere_mtv(
            a_center,
            radius,
            half_segment,
            b_center,
            b,
            extra,
            inclusive,
        ),
        (
            Sphere { radius },
            CapsuleY {
                radius: b,
                half_segment,
            },
        ) => invert(capsule_sphere_mtv(
            b_center,
            b,
            half_segment,
            a_center,
            radius,
            extra,
            inclusive,
        )),
        (
            CapsuleY {
                radius: a,
                half_segment: ah,
            },
            CapsuleY {
                radius: b,
                half_segment: bh,
            },
        ) => capsule_capsule_mtv(a_center, a, ah, b_center, b, bh, extra, inclusive),
        (
            CapsuleY {
                radius,
                half_segment,
            },
            Cube { half_extents },
        ) => capsule_box_mtv(
            a_center,
            radius,
            half_segment,
            b_center,
            half_extents,
            extra,
            inclusive,
        ),
        (
            Cube { half_extents },
            CapsuleY {
                radius,
                half_segment,
            },
        ) => invert(capsule_box_mtv(
            b_center,
            radius,
            half_segment,
            a_center,
            half_extents,
            extra,
            inclusive,
        )),
        (Sphere { radius }, Cube { half_extents }) => capsule_box_mtv(
            a_center,
            radius,
            0.0,
            b_center,
            half_extents,
            extra,
            inclusive,
        ),
        (Cube { half_extents }, Sphere { radius }) => invert(capsule_box_mtv(
            b_center,
            radius,
            0.0,
            a_center,
            half_extents,
            extra,
            inclusive,
        )),
    }
}

fn invert(v: Option<[f32; 3]>) -> Option<[f32; 3]> {
    v.map(|v| [-v[0], -v[1], -v[2]])
}

fn accepts(depth: f32, inclusive: bool) -> bool {
    if inclusive {
        depth >= -GEOMETRY_EPSILON
    } else {
        depth > GEOMETRY_EPSILON
    }
}

fn box_box_mtv(
    ac: [f32; 3],
    ae: [f32; 3],
    bc: [f32; 3],
    be: [f32; 3],
    extra: f32,
    inclusive: bool,
) -> Option<[f32; 3]> {
    let mut best = (f32::INFINITY, 0usize, 1.0f32);
    for axis in 0..3 {
        let delta = ac[axis] - bc[axis];
        let depth = ae[axis] + be[axis] - delta.abs();
        if !accepts(depth, inclusive) {
            return None;
        }
        if depth < best.0 {
            best = (depth, axis, if delta < 0.0 { -1.0 } else { 1.0 });
        }
    }
    let mut out = [0.0; 3];
    out[best.1] = best.2 * (best.0.max(0.0) + extra);
    Some(out)
}

fn sphere_sphere_mtv(
    ac: [f32; 3],
    ar: f32,
    bc: [f32; 3],
    br: f32,
    extra: f32,
    inclusive: bool,
) -> Option<[f32; 3]> {
    radial_mtv(sub(ac, bc), ar + br, extra, inclusive, [1.0, 0.0, 0.0])
}

fn capsule_sphere_mtv(
    cc: [f32; 3],
    cr: f32,
    half: f32,
    sc: [f32; 3],
    sr: f32,
    extra: f32,
    inclusive: bool,
) -> Option<[f32; 3]> {
    let segment_y = sc[1].clamp(cc[1] - half, cc[1] + half);
    radial_mtv(
        [cc[0] - sc[0], segment_y - sc[1], cc[2] - sc[2]],
        cr + sr,
        extra,
        inclusive,
        if cc[1] < sc[1] {
            [0.0, -1.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        },
    )
}

fn capsule_capsule_mtv(
    ac: [f32; 3],
    ar: f32,
    ah: f32,
    bc: [f32; 3],
    br: f32,
    bh: f32,
    extra: f32,
    inclusive: bool,
) -> Option<[f32; 3]> {
    let a_min = ac[1] - ah;
    let a_max = ac[1] + ah;
    let b_min = bc[1] - bh;
    let b_max = bc[1] + bh;
    let (ay, by) = if a_max < b_min {
        (a_max, b_min)
    } else if b_max < a_min {
        (a_min, b_max)
    } else {
        let y = ac[1].clamp(b_min, b_max).clamp(a_min, a_max);
        (y, y)
    };
    let delta = [ac[0] - bc[0], ay - by, ac[2] - bc[2]];
    let radial = radial_mtv(delta, ar + br, extra, inclusive, [1.0, 0.0, 0.0])?;
    if delta[0].abs() <= GEOMETRY_EPSILON
        && delta[1].abs() <= GEOMETRY_EPSILON
        && delta[2].abs() <= GEOMETRY_EPSILON
    {
        let y_delta = ac[1] - bc[1];
        let y_depth = ah + ar + bh + br - y_delta.abs();
        if y_depth < ar + br && accepts(y_depth, inclusive) {
            return Some([
                0.0,
                (if y_delta < 0.0 { -1.0 } else { 1.0 }) * (y_depth.max(0.0) + extra),
                0.0,
            ]);
        }
    }
    Some(radial)
}

fn capsule_box_mtv(
    cc: [f32; 3],
    radius: f32,
    half: f32,
    bc: [f32; 3],
    ext: [f32; 3],
    extra: f32,
    inclusive: bool,
) -> Option<[f32; 3]> {
    let bmin = [bc[0] - ext[0], bc[1] - ext[1], bc[2] - ext[2]];
    let bmax = [bc[0] + ext[0], bc[1] + ext[1], bc[2] + ext[2]];
    let seg_min = cc[1] - half;
    let seg_max = cc[1] + half;
    let seg_y = if seg_max < bmin[1] {
        seg_max
    } else if seg_min > bmax[1] {
        seg_min
    } else {
        cc[1].clamp(bmin[1], bmax[1]).clamp(seg_min, seg_max)
    };
    let box_point = [
        cc[0].clamp(bmin[0], bmax[0]),
        seg_y.clamp(bmin[1], bmax[1]),
        cc[2].clamp(bmin[2], bmax[2]),
    ];
    let delta = [
        cc[0] - box_point[0],
        seg_y - box_point[1],
        cc[2] - box_point[2],
    ];
    if length_squared(delta) > GEOMETRY_EPSILON * GEOMETRY_EPSILON {
        return radial_mtv(delta, radius, extra, inclusive, [1.0, 0.0, 0.0]);
    }

    // The capsule segment pierces the box. Pick the cheapest face of the box
    // expanded by the capsule radius; this gives stable floor/ceiling normals.
    let candidates = [
        (cc[0] - (bmin[0] - radius), [-1.0, 0.0, 0.0]),
        ((bmax[0] + radius) - cc[0], [1.0, 0.0, 0.0]),
        (seg_max - (bmin[1] - radius), [0.0, -1.0, 0.0]),
        ((bmax[1] + radius) - seg_min, [0.0, 1.0, 0.0]),
        (cc[2] - (bmin[2] - radius), [0.0, 0.0, -1.0]),
        ((bmax[2] + radius) - cc[2], [0.0, 0.0, 1.0]),
    ];
    let (depth, normal) = candidates.into_iter().min_by(|a, b| a.0.total_cmp(&b.0))?;
    if !accepts(depth, inclusive) {
        return None;
    }
    Some(scale(normal, depth.max(0.0) + extra))
}

fn radial_mtv(
    delta: [f32; 3],
    radius: f32,
    extra: f32,
    inclusive: bool,
    fallback: [f32; 3],
) -> Option<[f32; 3]> {
    let distance = length_squared(delta).sqrt();
    let depth = radius - distance;
    if !accepts(depth, inclusive) {
        return None;
    }
    let normal = if distance > GEOMETRY_EPSILON {
        scale(delta, 1.0 / distance)
    } else {
        fallback
    };
    Some(scale(normal, depth.max(0.0) + extra))
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn length_squared(v: [f32; 3]) -> f32 {
    v[0] * v[0] + v[1] * v[1] + v[2] * v[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capsule_aabb_is_upright() {
        assert_eq!(
            world_aabb([1.0, 2.0, 3.0], CollisionShape::capsule_y(0.5, 1.0)),
            ([0.5, 0.5, 2.5], [1.5, 3.5, 3.5])
        );
    }

    #[test]
    fn capsule_pairs_are_inclusive_and_mtv_is_symmetric() {
        let capsule = CollisionShape::capsule_y(0.5, 1.0);
        let cube = CollisionShape::cube_half_extents([1.0, 0.5, 1.0]);
        assert!(intersects([0.0, 2.0, 0.0], capsule, [0.0, 0.0, 0.0], cube));
        assert!(
            minimum_translation([0.0, 2.0, 0.0], capsule, [0.0, 0.0, 0.0], cube, 0.0).is_none()
        );

        let a = [1.2, 0.0, 1.2];
        let ab = minimum_translation(a, capsule, [0.0; 3], cube, 0.0).unwrap();
        let ba = minimum_translation([0.0; 3], cube, a, capsule, 0.0).unwrap();
        assert!(ab.iter().zip(ba).all(|(a, b)| (*a + b).abs() < 1e-5));
        assert!(ab[0] > 0.0 && ab[2] > 0.0);
    }

    #[test]
    fn coincident_capsules_have_deterministic_fallback() {
        let c = CollisionShape::capsule_y(0.25, 0.75);
        assert_eq!(
            minimum_translation([0.0; 3], c, [0.0; 3], c, 0.0),
            Some([0.5, 0.0, 0.0])
        );
    }

    #[test]
    fn every_capsule_pair_works_in_both_orders() {
        let capsule = CollisionShape::capsule_y(0.4, 0.8);
        let sphere = CollisionShape::sphere_radius(0.5);
        let cube = CollisionShape::cube_half_extents([0.5, 0.5, 0.5]);
        for (other, center) in [(sphere, [0.6, 0.0, 0.0]), (cube, [0.7, 0.0, 0.0])] {
            assert!(intersects([0.0; 3], capsule, center, other));
            assert!(intersects(center, other, [0.0; 3], capsule));
            let ab = minimum_translation([0.0; 3], capsule, center, other, 0.0).unwrap();
            let ba = minimum_translation(center, other, [0.0; 3], capsule, 0.0).unwrap();
            assert!(ab.iter().zip(ba).all(|(a, b)| (*a + b).abs() < 1e-5));
        }

        assert!(!intersects([0.0; 3], capsule, [2.0, 0.0, 0.0], sphere));
        assert!(intersects([0.0; 3], capsule, [0.9, 0.0, 0.0], sphere));
        assert!(minimum_translation([0.0; 3], capsule, [0.9, 0.0, 0.0], sphere, 0.0).is_none());
    }

    #[test]
    fn capsule_box_floor_ceiling_wall_and_containment_normals_are_stable() {
        let capsule = CollisionShape::capsule_y(0.25, 0.75);
        let floor = CollisionShape::cube_half_extents([5.0, 0.5, 5.0]);
        let down = minimum_translation([0.0, 1.45, 0.0], capsule, [0.0; 3], floor, 0.0).unwrap();
        let up = minimum_translation([0.0, -1.45, 0.0], capsule, [0.0; 3], floor, 0.0).unwrap();
        assert!(down[1] > 0.0 && down[0] == 0.0 && down[2] == 0.0);
        assert!(up[1] < 0.0 && up[0] == 0.0 && up[2] == 0.0);

        let wall = CollisionShape::cube_half_extents([0.5, 5.0, 5.0]);
        let side = minimum_translation([0.7, 0.0, 0.0], capsule, [0.0; 3], wall, 0.0).unwrap();
        assert!(side[0] > 0.0 && side[1] == 0.0 && side[2] == 0.0);

        let contained = minimum_translation([0.0; 3], capsule, [0.0; 3], floor, 0.0).unwrap();
        assert!(contained.iter().any(|v| v.abs() > 0.0));
    }
}
