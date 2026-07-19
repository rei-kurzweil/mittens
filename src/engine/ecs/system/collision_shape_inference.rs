//! Reusable render-bounds-to-collision-shape inference heuristics.

use crate::engine::ecs::component::CollisionShape;
use crate::engine::graphics::bounds::Aabb;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InferredUprightCapsule {
    pub center_y: f32,
    pub shape: CollisionShape,
}

/// Infer an upright character capsule from aggregate vertical bounds.
/// Horizontal bounds are deliberately ignored so a T-pose cannot widen it.
pub fn infer_upright_capsule(
    bounds: &Aabb,
    authored_radius: f32,
) -> Option<InferredUprightCapsule> {
    let min_y = bounds.min[1];
    let max_y = bounds.max[1];
    let height = max_y - min_y;
    if !min_y.is_finite() || !max_y.is_finite() || height < 0.0 {
        return None;
    }
    let radius = authored_radius.max(0.0).min(height * 0.5);
    Some(InferredUprightCapsule {
        center_y: (min_y + max_y) * 0.5,
        shape: CollisionShape::capsule_y(radius, height * 0.5 - radius),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_only_height_and_caps_radius_for_small_avatar() {
        let narrow = Aabb {
            min: [-0.2, -1.0, -0.1],
            max: [0.2, 2.0, 0.1],
        };
        let wide = Aabb {
            min: [-20.0, -1.0, -7.0],
            max: [20.0, 2.0, 7.0],
        };
        assert_eq!(
            infer_upright_capsule(&narrow, 0.28),
            infer_upright_capsule(&wide, 0.28)
        );

        let tiny = Aabb {
            min: [0.0, -0.1, 0.0],
            max: [0.0, 0.3, 0.0],
        };
        let inferred = infer_upright_capsule(&tiny, 0.28).unwrap();
        assert!((inferred.center_y - 0.1).abs() < 1e-6);
        assert_eq!(inferred.shape, CollisionShape::capsule_y(0.2, 0.0));
    }
}
