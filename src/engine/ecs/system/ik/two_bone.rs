//! Positional triangle math shared by arm and lower-body policies.
use crate::utils::math::{vec3_add, vec3_cross, vec3_normalize, vec3_scale};

/// Caller chooses a feasible reach and bend plane. No hand/forearm twist policy.
pub(crate) fn midpoint(
    root: [f32; 3],
    direction: [f32; 3],
    normal: [f32; 3],
    upper: f32,
    lower: f32,
    reach: f32,
) -> [f32; 3] {
    vec3_add(
        root,
        vec3_scale(
            bend_direction(direction, normal, upper, lower, reach),
            upper,
        ),
    )
}

pub(crate) fn bend_direction(
    direction: [f32; 3],
    normal: [f32; 3],
    upper: f32,
    lower: f32,
    reach: f32,
) -> [f32; 3] {
    let angle = ((upper * upper + reach * reach - lower * lower) / (2.0 * upper * reach))
        .clamp(-1.0, 1.0)
        .acos();
    let perpendicular = vec3_normalize(vec3_cross(normal, direction));
    vec3_normalize(vec3_add(
        vec3_scale(direction, angle.cos()),
        vec3_scale(perpendicular, angle.sin()),
    ))
}
