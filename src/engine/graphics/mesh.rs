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

    /// Cone centered at origin, axis-aligned along +Z.
    ///
    /// Geometry:
    /// - height = 1.0 (z in [-0.5, +0.5])
    /// - base radius = 0.5 (at z = -0.5)
    ///
    /// `number_of_segments` controls radial tessellation.
    pub fn cone(number_of_segments: u32) -> CpuMesh {
        let segs = number_of_segments.max(3);
        let radius = 0.5_f32;
        let z_base = -0.5_f32;
        let z_tip = 0.5_f32;

        let tip = [0.0_f32, 0.0_f32, z_tip];
        let base_center = [0.0_f32, 0.0_f32, z_base];

        let mut vertices: Vec<CpuVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        fn vec3_sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
            [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
        }

        fn vec3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        }

        fn vec3_len(v: [f32; 3]) -> f32 {
            (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
        }

        fn vec3_normalize(v: [f32; 3]) -> [f32; 3] {
            let len = vec3_len(v);
            if len > 0.0 {
                [v[0] / len, v[1] / len, v[2] / len]
            } else {
                [0.0, 0.0, 1.0]
            }
        }

        // Side faces: flat-shaded (unique verts per triangle).
        for i in 0..segs {
            let a0 = (i as f32) / (segs as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32) / (segs as f32) * std::f32::consts::TAU;
            let (s0, c0) = a0.sin_cos();
            let (s1, c1) = a1.sin_cos();

            let p0 = [c0 * radius, s0 * radius, z_base];
            let p1 = [c1 * radius, s1 * radius, z_base];

            // Normal from triangle (tip, p0, p1).
            let e0 = vec3_sub(p0, tip);
            let e1 = vec3_sub(p1, tip);
            let n = vec3_normalize(vec3_cross(e0, e1));

            let base = vertices.len() as u32;
            vertices.push(CpuVertex {
                pos: tip,
                uv: [0.5, 0.0],
                normal: n,
            });
            vertices.push(CpuVertex {
                pos: p0,
                uv: [0.0, 1.0],
                normal: n,
            });
            vertices.push(CpuVertex {
                pos: p1,
                uv: [1.0, 1.0],
                normal: n,
            });
            indices.extend_from_slice(&[base, base + 1, base + 2]);
        }

        // Base cap: triangles wound CCW when viewed from -Z (outside).
        let n_base = [0.0_f32, 0.0_f32, -1.0_f32];
        for i in 0..segs {
            let a0 = (i as f32) / (segs as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32) / (segs as f32) * std::f32::consts::TAU;
            let (s0, c0) = a0.sin_cos();
            let (s1, c1) = a1.sin_cos();

            let p0 = [c0 * radius, s0 * radius, z_base];
            let p1 = [c1 * radius, s1 * radius, z_base];

            let base = vertices.len() as u32;
            vertices.push(CpuVertex {
                pos: base_center,
                uv: [0.5, 0.5],
                normal: n_base,
            });
            vertices.push(CpuVertex {
                pos: p1,
                uv: [0.5 + p1[0], 0.5 - p1[1]],
                normal: n_base,
            });
            vertices.push(CpuVertex {
                pos: p0,
                uv: [0.5 + p0[0], 0.5 - p0[1]],
                normal: n_base,
            });

            indices.extend_from_slice(&[base, base + 1, base + 2]);
        }

        CpuMesh::new(vertices, indices)
    }

    /// 2D ring/annulus in the XY plane (normal +Z).
    ///
    /// `inner_radius` and `outer_radius` are in object-space units.
    pub fn circle_2d(inner_radius: f32, outer_radius: f32, number_of_segments: u32) -> CpuMesh {
        let segs = number_of_segments.max(3);
        let inner = inner_radius.max(0.0);
        let outer = outer_radius.max(inner + 1.0e-6);
        let n = [0.0_f32, 0.0_f32, 1.0_f32];

        let mut vertices: Vec<CpuVertex> = Vec::with_capacity((segs as usize) * 2);
        let mut indices: Vec<u32> = Vec::with_capacity((segs as usize) * 6);

        // Outer ring vertices.
        for i in 0..segs {
            let a = (i as f32) / (segs as f32) * std::f32::consts::TAU;
            let (s, c) = a.sin_cos();
            let x = c * outer;
            let y = s * outer;
            let uv = [0.5 + x / (2.0 * outer), 0.5 - y / (2.0 * outer)];
            vertices.push(CpuVertex {
                pos: [x, y, 0.0],
                uv,
                normal: n,
            });
        }
        // Inner ring vertices.
        for i in 0..segs {
            let a = (i as f32) / (segs as f32) * std::f32::consts::TAU;
            let (s, c) = a.sin_cos();
            let x = c * inner;
            let y = s * inner;
            let uv = [0.5 + x / (2.0 * outer), 0.5 - y / (2.0 * outer)];
            vertices.push(CpuVertex {
                pos: [x, y, 0.0],
                uv,
                normal: n,
            });
        }

        // Indices (two triangles per segment).
        for i in 0..segs {
            let next = (i + 1) % segs;
            let outer_i = i;
            let outer_n = next;
            let inner_i = segs + i;
            let inner_n = segs + next;

            // Quad: outer_i -> outer_n -> inner_n -> inner_i
            indices.extend_from_slice(&[outer_i, outer_n, inner_n]);
            indices.extend_from_slice(&[outer_i, inner_n, inner_i]);
        }

        CpuMesh::new(vertices, indices)
    }
}
