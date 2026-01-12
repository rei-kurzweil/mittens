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
/// - `uv`: optional 0..1 UV (useful for screen-space gradients)
#[derive(BufferContents, Vertex, Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct CpuVertex {
    #[format(R32G32B32_SFLOAT)]
    pub pos: [f32; 3],
    #[format(R32G32_SFLOAT)]
    pub uv: [f32; 2],
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
    pub primitive_topology: PrimitiveTopology,
    pub index_format: IndexFormat,
}

impl CpuMesh {
    pub fn new(vertices: Vec<CpuVertex>, indices_u32: Vec<u32>) -> Self {
        Self {
            vertices,
            indices_u32,
            primitive_topology: PrimitiveTopology::TriangleList,
            index_format: IndexFormat::U32,
        }
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
///   (If your Vulkan pipeline uses CLOCKWISE front_face, you may need to flip.)
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
            },
            CpuVertex {
                pos: [0.5, y_bottom, 0.0],
                uv: [1.0, 0.0],
            },
            CpuVertex {
                pos: [0.0, y_top, 0.0],
                uv: [0.5, (y_top - y_bottom) / y_span],
            },
        ];

        CpuMesh::new(vertices, vec![0, 1, 2])
    }

    /// 2D quad (square) as two triangles.
    pub fn quad_2d() -> CpuMesh {
        let vertices = vec![
            CpuVertex {
                pos: [-0.5, -0.5, 0.0],
                uv: [0.0, 0.0],
            },
            CpuVertex {
                pos: [0.5, -0.5, 0.0],
                uv: [1.0, 0.0],
            },
            CpuVertex {
                pos: [0.5, 0.5, 0.0],
                uv: [1.0, 1.0],
            },
            CpuVertex {
                pos: [-0.5, 0.5, 0.0],
                uv: [0.0, 1.0],
            },
        ];

        // two triangles: (0,1,2) + (0,2,3)
        CpuMesh::new(vertices, vec![0, 1, 2, 0, 2, 3])
    }

    /// Unit-ish cube centered at origin.
    ///
    /// This is an *indexed position-only* cube (8 vertices, 12 triangles).
    pub fn cube() -> CpuMesh {
        let v = |x: f32, y: f32, z: f32| CpuVertex {
            pos: [x, y, z],
            uv: [0.0, 0.0],
        };

        let vertices = vec![
            v(-0.5, -0.5, -0.5), // 0
            v(0.5, -0.5, -0.5),  // 1
            v(0.5, 0.5, -0.5),   // 2
            v(-0.5, 0.5, -0.5),  // 3
            v(-0.5, -0.5, 0.5),  // 4
            v(0.5, -0.5, 0.5),   // 5
            v(0.5, 0.5, 0.5),    // 6
            v(-0.5, 0.5, 0.5),   // 7
        ];

        // 12 triangles (2 per face), CCW when looking at the outside
        let indices = vec![
            // -Z face
            0, 2, 1, 0, 3, 2, // +Z face
            4, 5, 6, 4, 6, 7, // -X face
            0, 4, 7, 0, 7, 3, // +X face
            1, 2, 6, 1, 6, 5, // -Y face
            0, 1, 5, 0, 5, 4, // +Y face
            3, 7, 6, 3, 6, 2,
        ];

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
            },
            CpuVertex {
                pos: [-0.5, -0.2886751, -0.2041241],
                uv: [0.0, 0.0],
            },
            CpuVertex {
                pos: [0.5, -0.2886751, -0.2041241],
                uv: [1.0, 0.0],
            },
            CpuVertex {
                pos: [0.0, 0.5773503, -0.2041241],
                uv: [0.5, 0.5],
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
}
