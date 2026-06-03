//! Local-space AABBs for built-in primitive meshes.
//!
//! Single source of truth for "what extents does mesh X cover in its own
//! local coordinate frame". Consumed by `BvhSystem` (which transforms by
//! `matrix_world` for raycast) and by `BoundsComponent` (which caches the
//! local extents directly for layout-time intrinsic sizing).

use crate::engine::graphics::primitives::{CpuMeshHandle, TransformMatrix};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Aabb {
    pub fn from_points(pts: &[[f32; 3]]) -> Option<Self> {
        if pts.is_empty() {
            return None;
        }
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for p in pts {
            for i in 0..3 {
                if p[i] < min[i] {
                    min[i] = p[i];
                }
                if p[i] > max[i] {
                    max[i] = p[i];
                }
            }
        }
        Some(Self { min, max })
    }

    pub fn width(&self) -> f32 {
        self.max[0] - self.min[0]
    }
    pub fn height(&self) -> f32 {
        self.max[1] - self.min[1]
    }
    pub fn depth(&self) -> f32 {
        self.max[2] - self.min[2]
    }

    pub fn inflated_z(mut self, half: f32) -> Self {
        self.min[2] -= half;
        self.max[2] += half;
        self
    }

    pub fn union(&self, other: &Self) -> Self {
        let mut min = [0.0; 3];
        let mut max = [0.0; 3];
        for i in 0..3 {
            min[i] = self.min[i].min(other.min[i]);
            max[i] = self.max[i].max(other.max[i]);
        }
        Self { min, max }
    }

    /// Transform all 8 corners by `m` and return the axis-aligned bound of the
    /// result. For arbitrary affine `m` the returned AABB may be looser than
    /// the source — that's the price of axis alignment.
    pub fn transformed(&self, m: TransformMatrix) -> Self {
        let corners = [
            [self.min[0], self.min[1], self.min[2]],
            [self.max[0], self.min[1], self.min[2]],
            [self.min[0], self.max[1], self.min[2]],
            [self.max[0], self.max[1], self.min[2]],
            [self.min[0], self.min[1], self.max[2]],
            [self.max[0], self.min[1], self.max[2]],
            [self.min[0], self.max[1], self.max[2]],
            [self.max[0], self.max[1], self.max[2]],
        ];
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for c in &corners {
            let v = mul_mat4_vec4(m, [c[0], c[1], c[2], 1.0]);
            for i in 0..3 {
                if v[i] < min[i] {
                    min[i] = v[i];
                }
                if v[i] > max[i] {
                    max[i] = v[i];
                }
            }
        }
        Self { min, max }
    }

    pub fn center(&self) -> [f32; 3] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
            (self.min[2] + self.max[2]) * 0.5,
        ]
    }

    pub fn max_dimension(&self) -> f32 {
        self.width().max(self.height()).max(self.depth())
    }
}

pub fn mat4_identity() -> TransformMatrix {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub fn mat4_mul(a: TransformMatrix, b: TransformMatrix) -> TransformMatrix {
    let mut out = [[0.0f32; 4]; 4];
    for c in 0..4 {
        for r in 0..4 {
            out[c][r] =
                a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2] + a[3][r] * b[c][3];
        }
    }
    out
}

fn mul_mat4_vec4(m: TransformMatrix, v: [f32; 4]) -> [f32; 4] {
    [
        m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2] + m[3][0] * v[3],
        m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2] + m[3][1] * v[3],
        m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2] + m[3][2] * v[3],
        m[0][3] * v[0] + m[1][3] * v[1] + m[2][3] * v[2] + m[3][3] * v[3],
    ]
}

/// Local-space AABB for a known built-in `CpuMeshHandle`.
///
/// Returns `None` for meshes whose extents aren't tabulated here (notably
/// GLTF-loaded meshes — those need their bounds computed from vertex data
/// at load time, which is downstream work).
pub fn mesh_local_aabb(mesh: CpuMeshHandle) -> Option<Aabb> {
    match mesh {
        CpuMeshHandle::CUBE => Aabb::from_points(&[
            [-0.5, -0.5, -0.5],
            [0.5, -0.5, -0.5],
            [-0.5, 0.5, -0.5],
            [0.5, 0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [-0.5, 0.5, 0.5],
            [0.5, 0.5, 0.5],
        ]),
        CpuMeshHandle::TETRAHEDRON => Aabb::from_points(&[
            [0.0, 0.0, 0.6123724],
            [-0.5, -0.2886751, -0.2041241],
            [0.5, -0.2886751, -0.2041241],
            [0.0, 0.5773503, -0.2041241],
        ]),
        CpuMeshHandle::CONE => Aabb::from_points(&[
            [-0.5, 0.0, -0.5],
            [0.5, 0.0, -0.5],
            [0.0, -0.5, -0.5],
            [0.0, 0.5, -0.5],
            [0.0, 0.0, 0.5],
        ]),
        CpuMeshHandle::QUAD_2D | CpuMeshHandle::TRIANGLE_2D | CpuMeshHandle::CIRCLE_2D => {
            // 2D primitives are flat on z=0; thicken slightly so raycast / BVH
            // produce a non-degenerate AABB.
            Aabb::from_points(&[
                [-0.5, -0.5, 0.0],
                [0.5, -0.5, 0.0],
                [-0.5, 0.5, 0.0],
                [0.5, 0.5, 0.0],
            ])
            .map(|a| a.inflated_z(0.01))
        }
        CpuMeshHandle::SPHERE => Aabb::from_points(&[
            [-0.5, 0.0, 0.0],
            [0.5, 0.0, 0.0],
            [0.0, -0.5, 0.0],
            [0.0, 0.5, 0.0],
            [0.0, 0.0, -0.5],
            [0.0, 0.0, 0.5],
        ]),
        _ => None,
    }
}
