use crate::engine::ecs::component::TransformFilterComponent;
use crate::engine::graphics::primitives::TransformMatrix;
use crate::utils::math;

#[derive(Debug, Default)]
pub struct TransformFilterSystem;

impl TransformFilterSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn filter_inherited_world(parent_world: TransformMatrix, filter: &TransformFilterComponent) -> TransformMatrix {
        fn col3(m: TransformMatrix, c: usize) -> [f32; 3] {
            [m[c][0], m[c][1], m[c][2]]
        }

        let mut out: TransformMatrix = math::mat4_identity();

        // Translation.
        if filter.inherit_translation {
            out[3][0] = parent_world[3][0];
            out[3][1] = parent_world[3][1];
            out[3][2] = parent_world[3][2];
        }

        // Basis.
        let b0 = col3(parent_world, 0);
        let b1 = col3(parent_world, 1);
        let b2 = col3(parent_world, 2);

        let sx = math::vec3_len(b0).max(1e-8);
        let sy = math::vec3_len(b1).max(1e-8);
        let sz = math::vec3_len(b2).max(1e-8);

        match (filter.inherit_rotation, filter.inherit_scale) {
            (true, true) => {
                // Inherit full basis (rotation + scale as encoded).
                out[0] = parent_world[0];
                out[1] = parent_world[1];
                out[2] = parent_world[2];
            }
            (true, false) => {
                // Keep rotation only: drop scale and re-orthonormalize.
                let x = math::vec3_normalize(b0);
                let y_proj = math::vec3_sub(b1, math::vec3_scale(x, math::vec3_dot(b1, x)));
                let mut y = math::vec3_normalize(y_proj);
                let mut z = math::vec3_cross(x, y);
                if math::vec3_len(z) < 1e-6 {
                    // Degenerate: fall back to using the third basis and try to build a frame.
                    z = math::vec3_normalize(b2);
                    y = math::vec3_normalize(math::vec3_cross(z, x));
                } else {
                    z = math::vec3_normalize(z);
                    // Recompute y to enforce orthogonality.
                    y = math::vec3_normalize(math::vec3_cross(z, x));
                }

                out[0][0] = x[0];
                out[0][1] = x[1];
                out[0][2] = x[2];

                out[1][0] = y[0];
                out[1][1] = y[1];
                out[1][2] = y[2];

                out[2][0] = z[0];
                out[2][1] = z[1];
                out[2][2] = z[2];
            }
            (false, true) => {
                // Keep scale only: axis-aligned scale.
                out[0][0] = sx;
                out[1][1] = sy;
                out[2][2] = sz;
            }
            (false, false) => {
                // Identity basis.
            }
        }

        out
    }
}
