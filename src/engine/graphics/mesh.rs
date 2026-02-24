//! CPU-side procedural mesh generation.
//!
//! These meshes are intended as authoring / staging data.
//! The renderer later uploads them into GPU buffers (vertex/index buffers)
//! and returns a `MeshHandle` that can be referenced by ECS renderables.

use vulkano::buffer::BufferContents;
use vulkano::pipeline::graphics::vertex_input::Vertex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveTopology {
    TriangleList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexFormat {
    U16,
    U32,
}

/// A minimal CPU vertex format for bring-up.
///
/// - `pos`: object-space / model-space position
/// - `normal`: object-space normal (for lighting)
/// - `uv`: optional 0..1 UV (useful for screen-space gradients)
#[derive(BufferContents, Vertex, Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct CpuVertex {
    #[format(R32G32B32_SFLOAT)]
    pub pos: [f32; 3],
    #[format(R32G32_SFLOAT)]
    pub uv: [f32; 2],
    #[format(R32G32B32_SFLOAT)]
    pub normal: [f32; 3],
}

/// CPU-side mesh data.
///
/// Contract:
/// - `vertices` + `indices` fully define geometry.
/// - `primitive_topology` is how indices are interpreted.
/// - Upload step will pack `vertices` as tightly as possible into a GPU vertex buffer,
///   and `indices` into a GPU index buffer.
#[derive(Debug, Clone)]
pub struct CpuMesh {
    pub vertices: Vec<CpuVertex>,
    pub indices_u32: Vec<u32>,

    /// Optional skinning data (glTF: `JOINTS_0` / `WEIGHTS_0`).
    ///
    /// Contract (when present):
    /// - `len == vertices.len()`
    /// - `joints0[i]` corresponds to `weights0[i]`
    pub joints0: Option<Vec<[u16; 4]>>,
    pub weights0: Option<Vec<[f32; 4]>>,
    pub primitive_topology: PrimitiveTopology,
    pub index_format: IndexFormat,
}

impl CpuMesh {
    pub fn new(vertices: Vec<CpuVertex>, indices_u32: Vec<u32>) -> Self {
        Self {
            vertices,
            indices_u32,
            joints0: None,
            weights0: None,
            primitive_topology: PrimitiveTopology::TriangleList,
            index_format: IndexFormat::U32,
        }
    }

    pub fn with_skinning(mut self, joints0: Vec<[u16; 4]>, weights0: Vec<[f32; 4]>) -> Self {
        debug_assert_eq!(joints0.len(), self.vertices.len());
        debug_assert_eq!(weights0.len(), self.vertices.len());
        debug_assert_eq!(joints0.len(), weights0.len());
        self.joints0 = Some(joints0);
        self.weights0 = Some(weights0);
        self
    }

    pub fn index_count(&self) -> u32 {
        self.indices_u32.len() as u32
    }

    pub fn vertex_count(&self) -> u32 {
        self.vertices.len() as u32
    }
}

/// Procedural mesh constructors.
///
/// Notes:
/// - The shapes here are intentionally simple and low-poly.
/// - Winding order:
///   We return *counter-clockwise* triangles in object space for "front faces".
pub struct MeshFactory;

impl MeshFactory {
    /// 2D equilateral triangle centered at origin.
    pub fn triangle_2d() -> CpuMesh {
        // Equilateral triangle of side length 1.0.
        // Height h = sqrt(3)/2. Centered at origin using:
        //  - top:    (0,  2h/3)
        //  - bottom: (±0.5, -h/3)
        let h = 0.866_025_4_f32;
        let y_top = 2.0 * h / 3.0;
        let y_bottom = -h / 3.0;
        let y_span = y_top - y_bottom;

        let vertices = vec![
            CpuVertex {
                pos: [-0.5, y_bottom, 0.0],
                // For 2D primitives, we treat UV as normalized XY over the primitive's bounds.
                uv: [0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
            },
            CpuVertex {
                pos: [0.5, y_bottom, 0.0],
                uv: [1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
            },
            CpuVertex {
                pos: [0.0, y_top, 0.0],
                uv: [0.5, (y_top - y_bottom) / y_span],
                normal: [0.0, 0.0, 1.0],
            },
        ];

        CpuMesh::new(vertices, vec![0, 1, 2])
    }

    /// 2D quad (square) as two triangles.
    pub fn quad_2d() -> CpuMesh {
        let vertices = vec![
            CpuVertex {
                pos: [-0.5, -0.5, 0.0],
                // Texture convention: v=0 is TOP, v=1 is BOTTOM.
                uv: [0.0, 1.0],
                normal: [0.0, 0.0, 1.0],
            },
            CpuVertex {
                pos: [0.5, -0.5, 0.0],
                uv: [1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
            },
            CpuVertex {
                pos: [0.5, 0.5, 0.0],
                uv: [1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
            },
            CpuVertex {
                pos: [-0.5, 0.5, 0.0],
                uv: [0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
            },
        ];

        // two triangles: (0,1,2) + (0,2,3)
        CpuMesh::new(vertices, vec![0, 1, 2, 0, 2, 3])
    }

    /// Unit-ish cube centered at origin.
    ///
    /// This is a cube with per-face vertices (24 vertices, 12 triangles) so normals are flat.
    pub fn cube() -> CpuMesh {
        let p = 0.5_f32;

        // 4 verts per face. UVs are placeholder; cube texturing isn't a priority yet.
        let mut vertices: Vec<CpuVertex> = Vec::with_capacity(24);
        let mut indices: Vec<u32> = Vec::with_capacity(36);

        let mut push_face = |n: [f32; 3], a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]| {
            let base = vertices.len() as u32;
            vertices.push(CpuVertex {
                pos: a,
                uv: [0.0, 0.0],
                normal: n,
            });
            vertices.push(CpuVertex {
                pos: b,
                uv: [1.0, 0.0],
                normal: n,
            });
            vertices.push(CpuVertex {
                pos: c,
                uv: [1.0, 1.0],
                normal: n,
            });
            vertices.push(CpuVertex {
                pos: d,
                uv: [0.0, 1.0],
                normal: n,
            });
            // CCW triangles as seen from outside.
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        };

        // -Z
        push_face(
            [0.0, 0.0, -1.0],
            [-p, -p, -p],
            [p, -p, -p],
            [p, p, -p],
            [-p, p, -p],
        );
        // +Z
        push_face(
            [0.0, 0.0, 1.0],
            [-p, -p, p],
            [p, -p, p],
            [p, p, p],
            [-p, p, p],
        );
        // -X
        push_face(
            [-1.0, 0.0, 0.0],
            [-p, -p, -p],
            [-p, -p, p],
            [-p, p, p],
            [-p, p, -p],
        );
        // +X
        push_face(
            [1.0, 0.0, 0.0],
            [p, -p, -p],
            [p, p, -p],
            [p, p, p],
            [p, -p, p],
        );
        // -Y
        push_face(
            [0.0, -1.0, 0.0],
            [-p, -p, -p],
            [p, -p, -p],
            [p, -p, p],
            [-p, -p, p],
        );
        // +Y
        push_face(
            [0.0, 1.0, 0.0],
            [-p, p, -p],
            [-p, p, p],
            [p, p, p],
            [p, p, -p],
        );

        CpuMesh::new(vertices, indices)
    }

    /// Simple tetrahedron (4 vertices, 4 faces).
    pub fn tetrahedron() -> CpuMesh {
        // A regular tetrahedron-ish set of points.
        // (Not perfectly regular, but stable and centered-ish.)
        let vertices = vec![
            CpuVertex {
                pos: [0.0, 0.0, 0.6123724],
                uv: [0.5, 1.0],
                normal: [0.0, 0.0, 1.0],
            },
            CpuVertex {
                pos: [-0.5, -0.2886751, -0.2041241],
                uv: [0.0, 0.0],
                normal: [-1.0, -1.0, -1.0],
            },
            CpuVertex {
                pos: [0.5, -0.2886751, -0.2041241],
                uv: [1.0, 0.0],
                normal: [1.0, -1.0, -1.0],
            },
            CpuVertex {
                pos: [0.0, 0.5773503, -0.2041241],
                uv: [0.5, 0.5],
                normal: [0.0, 1.0, -1.0],
            },
        ];

        // 4 faces, CCW as seen from outside.
        // NOTE: if these are wound the other way, the tetra renders “inside out”
        // under back-face culling.
        let indices = vec![
            0, 1, 2, // base-ish
            0, 3, 1, // side
            0, 2, 3, // side
            1, 3, 2, // bottom
        ];

        CpuMesh::new(vertices, indices)
    }

    /// UV sphere centered at origin.
    ///
    /// Radius is 0.5 to match the unit-ish cube extents.
    pub fn sphere() -> CpuMesh {
        let radius = 0.5_f32;
        let rings: u32 = 16;
        let segments: u32 = 32;

        let mut vertices: Vec<CpuVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        // Create vertices.
        // v in [0..rings], u in [0..segments]
        for r in 0..=rings {
            let v = r as f32 / rings as f32;
            let theta = v * std::f32::consts::PI; // 0..pi
            let (st, ct) = theta.sin_cos();

            for s in 0..=segments {
                let u = s as f32 / segments as f32;
                let phi = u * std::f32::consts::TAU; // 0..2pi
                let (sp, cp) = phi.sin_cos();

                let x = cp * st;
                let y = ct;
                let z = sp * st;

                vertices.push(CpuVertex {
                    pos: [x * radius, y * radius, z * radius],
                    uv: [u, 1.0 - v],
                    normal: [x, y, z],
                });
            }
        }

        // Create indices.
        let stride = segments + 1;
        for r in 0..rings {
            for s in 0..segments {
                let i0 = r * stride + s;
                let i1 = i0 + 1;
                let i2 = (r + 1) * stride + s;
                let i3 = i2 + 1;

                // Two triangles per quad.
                indices.extend_from_slice(&[i0, i2, i1]);
                indices.extend_from_slice(&[i1, i2, i3]);
            }
        }

        CpuMesh::new(vertices, indices)
    }
}
