//! CPU-side procedural mesh generation.
//!
//! These meshes are intended as authoring / staging data.
//! The renderer later uploads them into GPU buffers (vertex/index buffers)
//! and returns a `MeshHandle` that can be referenced by ECS renderables.

use vulkano::buffer::BufferContents;
use vulkano::pipeline::graphics::vertex_input::Vertex;

fn filled_polygon_2d(boundary: &[[f32; 2]]) -> CpuMesh {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for [x, y] in boundary {
        min_x = min_x.min(*x);
        max_x = max_x.max(*x);
        min_y = min_y.min(*y);
        max_y = max_y.max(*y);
    }
    let width = (max_x - min_x).max(1.0e-6);
    let height = (max_y - min_y).max(1.0e-6);

    let mut vertices = Vec::with_capacity(boundary.len() + 1);
    vertices.push(CpuVertex {
        pos: [0.0, 0.0, 0.0],
        uv: [
            ((0.0 - min_x) / width).clamp(0.0, 1.0),
            (1.0 - (0.0 - min_y) / height).clamp(0.0, 1.0),
        ],
        normal: [0.0, 0.0, 1.0],
    });
    for [x, y] in boundary {
        vertices.push(CpuVertex {
            pos: [*x, *y, 0.0],
            uv: [(*x - min_x) / width, 1.0 - (*y - min_y) / height],
            normal: [0.0, 0.0, 1.0],
        });
    }

    let mut indices = Vec::with_capacity(boundary.len() * 3);
    for i in 0..boundary.len() {
        let next = (i + 1) % boundary.len();
        indices.extend_from_slice(&[0, (i + 1) as u32, (next + 1) as u32]);
    }
    CpuMesh::new(vertices, indices)
}

fn signed_area_2d(points: &[[f32; 2]]) -> f32 {
    let mut area = 0.0;
    for i in 0..points.len() {
        let [x0, y0] = points[i];
        let [x1, y1] = points[(i + 1) % points.len()];
        area += x0 * y1 - x1 * y0;
    }
    area * 0.5
}

fn add2(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] + b[0], a[1] + b[1]]
}

fn sub2(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn mul2(v: [f32; 2], s: f32) -> [f32; 2] {
    [v[0] * s, v[1] * s]
}

fn len2(v: [f32; 2]) -> f32 {
    (v[0] * v[0] + v[1] * v[1]).sqrt()
}

fn normalize2(v: [f32; 2]) -> [f32; 2] {
    let len = len2(v).max(1.0e-6);
    [v[0] / len, v[1] / len]
}

fn quadratic_bezier2(a: [f32; 2], b: [f32; 2], c: [f32; 2], t: f32) -> [f32; 2] {
    let u = 1.0 - t;
    add2(add2(mul2(a, u * u), mul2(b, 2.0 * u * t)), mul2(c, t * t))
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn mul3(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn len3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = len3(v).max(1.0e-6);
    [v[0] / len, v[1] / len, v[2] / len]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn append_rounded_corner_points(
    boundary: &mut Vec<[f32; 2]>,
    prev: [f32; 2],
    curr: [f32; 2],
    next: [f32; 2],
    bevel_segments: u32,
    trim_fraction: f32,
) {
    if bevel_segments == 0 {
        boundary.push(curr);
        return;
    }

    let to_prev = sub2(prev, curr);
    let to_next = sub2(next, curr);
    let trim = len2(to_prev).min(len2(to_next)) * trim_fraction.clamp(0.0, 0.49);
    let start = add2(curr, mul2(normalize2(to_prev), trim));
    let end = add2(curr, mul2(normalize2(to_next), trim));

    for i in 0..=bevel_segments {
        let t = i as f32 / bevel_segments as f32;
        boundary.push(quadratic_bezier2(start, curr, end, t));
    }
}

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

    pub fn approximate_heap_bytes(&self) -> usize {
        let mut bytes = self.vertices.capacity() * std::mem::size_of::<CpuVertex>();
        bytes += self.indices_u32.capacity() * std::mem::size_of::<u32>();
        if let Some(joints0) = &self.joints0 {
            bytes += joints0.capacity() * std::mem::size_of::<[u16; 4]>();
        }
        if let Some(weights0) = &self.weights0 {
            bytes += weights0.capacity() * std::mem::size_of::<[f32; 4]>();
        }
        bytes
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
    /// Exact Y-aligned capsule with hemispherical ends and a cylindrical middle.
    pub fn capsule_y(
        radius: f32,
        half_segment: f32,
        radial_segments: u32,
        hemisphere_segments: u32,
    ) -> CpuMesh {
        let radius = radius.max(0.0);
        let half_segment = half_segment.max(0.0);
        let radial = radial_segments.max(3) as usize;
        let hemi = hemisphere_segments.max(2) as usize;
        let mut vertices = Vec::with_capacity((hemi * 2 + 2) * (radial + 1));
        let mut indices = Vec::with_capacity((hemi * 2 + 1) * radial * 6);
        let mut push_ring = |latitude: f32, center_y: f32| {
            let ring_radius = radius * latitude.cos();
            let normal_y = latitude.sin();
            let y = center_y + radius * normal_y;
            for segment in 0..=radial {
                let angle = std::f32::consts::TAU * segment as f32 / radial as f32;
                let (sin, cos) = angle.sin_cos();
                vertices.push(CpuVertex {
                    pos: [ring_radius * cos, y, ring_radius * sin],
                    uv: [
                        segment as f32 / radial as f32,
                        0.5 - latitude / std::f32::consts::PI,
                    ],
                    normal: [latitude.cos() * cos, normal_y, latitude.cos() * sin],
                });
            }
        };
        for i in 0..=hemi {
            let latitude =
                -std::f32::consts::FRAC_PI_2 + std::f32::consts::FRAC_PI_2 * i as f32 / hemi as f32;
            push_ring(latitude, -half_segment);
        }
        for i in 0..=hemi {
            let latitude = std::f32::consts::FRAC_PI_2 * i as f32 / hemi as f32;
            push_ring(latitude, half_segment);
        }
        let rings = hemi * 2 + 2;
        for ring in 0..rings - 1 {
            for segment in 0..radial {
                let a = (ring * (radial + 1) + segment) as u32;
                let b = a + (radial + 1) as u32;
                indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
            }
        }
        CpuMesh::new(vertices, indices)
    }
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

    /// Unit wireframe square centered in the XY plane, represented by four solid edges.
    ///
    /// `thickness` is relative to the unit square dimensions. Triangles keep the shape on
    /// the normal mesh rendering path while providing consistent, visible edge thickness.
    pub fn wireframe_square(thickness: f32) -> CpuMesh {
        let thickness = thickness.clamp(1.0e-4, 0.5);
        let inner = 0.5 - thickness;
        let vertices = vec![
            CpuVertex {
                pos: [-0.5, -0.5, 0.0],
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
            CpuVertex {
                pos: [-inner, -inner, 0.0],
                uv: [thickness, 1.0 - thickness],
                normal: [0.0, 0.0, 1.0],
            },
            CpuVertex {
                pos: [inner, -inner, 0.0],
                uv: [1.0 - thickness, 1.0 - thickness],
                normal: [0.0, 0.0, 1.0],
            },
            CpuVertex {
                pos: [inner, inner, 0.0],
                uv: [1.0 - thickness, thickness],
                normal: [0.0, 0.0, 1.0],
            },
            CpuVertex {
                pos: [-inner, inner, 0.0],
                uv: [thickness, thickness],
                normal: [0.0, 0.0, 1.0],
            },
        ];
        let indices = vec![
            0, 1, 5, 0, 5, 4, // bottom
            1, 2, 6, 1, 6, 5, // right
            2, 3, 7, 2, 7, 6, // top
            3, 0, 4, 3, 4, 7, // left
        ];

        CpuMesh::new(vertices, indices)
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

    /// Unit wireframe box centered at the origin, represented by twelve solid edge prisms.
    ///
    /// `thickness` is relative to the unit box dimensions and is clamped to `(0, 1]`. Using
    /// triangles instead of line primitives keeps the geometry compatible with the normal mesh
    /// rendering path and gives the edges visible thickness from every view direction.
    pub fn wireframe_box(thickness: f32) -> CpuMesh {
        let thickness = thickness.clamp(1.0e-4, 1.0);
        let edge_center = 0.5 - thickness * 0.5;
        let cube = Self::cube();
        let mut vertices = Vec::with_capacity(cube.vertices.len() * 12);
        let mut indices = Vec::with_capacity(cube.indices_u32.len() * 12);

        let mut append_prism = |center: [f32; 3], size: [f32; 3]| {
            let base = vertices.len() as u32;
            vertices.extend(cube.vertices.iter().map(|vertex| CpuVertex {
                pos: [
                    center[0] + vertex.pos[0] * size[0],
                    center[1] + vertex.pos[1] * size[1],
                    center[2] + vertex.pos[2] * size[2],
                ],
                uv: vertex.uv,
                normal: vertex.normal,
            }));
            indices.extend(cube.indices_u32.iter().map(|index| base + index));
        };

        for a in [-edge_center, edge_center] {
            for b in [-edge_center, edge_center] {
                append_prism([0.0, a, b], [1.0, thickness, thickness]);
                append_prism([a, 0.0, b], [thickness, 1.0, thickness]);
                append_prism([a, b, 0.0], [thickness, thickness, 1.0]);
            }
        }

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

    /// Icosahedron with optional recursive tessellation and spherical blending.
    ///
    /// `tessellations` is the number of recursive 4-way triangle splits.
    /// `sphericalness` blends from planar face subdivision (`0.0`) to an icosphere (`1.0`).
    pub fn icosahedron(tessellations: u32, sphericalness: f32) -> CpuMesh {
        let radius = 0.5_f32;
        let sphericalness = sphericalness.clamp(0.0, 1.0);
        let phi = (1.0 + 5.0_f32.sqrt()) * 0.5;

        let base_positions = [
            normalize3([-1.0, phi, 0.0]),
            normalize3([1.0, phi, 0.0]),
            normalize3([-1.0, -phi, 0.0]),
            normalize3([1.0, -phi, 0.0]),
            normalize3([0.0, -1.0, phi]),
            normalize3([0.0, 1.0, phi]),
            normalize3([0.0, -1.0, -phi]),
            normalize3([0.0, 1.0, -phi]),
            normalize3([phi, 0.0, -1.0]),
            normalize3([phi, 0.0, 1.0]),
            normalize3([-phi, 0.0, -1.0]),
            normalize3([-phi, 0.0, 1.0]),
        ]
        .map(|p| mul3(p, radius));

        let base_faces = [
            [0, 11, 5],
            [0, 5, 1],
            [0, 1, 7],
            [0, 7, 10],
            [0, 10, 11],
            [1, 5, 9],
            [5, 11, 4],
            [11, 10, 2],
            [10, 7, 6],
            [7, 1, 8],
            [3, 9, 4],
            [3, 4, 2],
            [3, 2, 6],
            [3, 6, 8],
            [3, 8, 9],
            [4, 9, 5],
            [2, 4, 11],
            [6, 2, 10],
            [8, 6, 7],
            [9, 8, 1],
        ];

        let mut triangles: Vec<[[f32; 3]; 3]> = base_faces
            .iter()
            .map(|face| {
                [
                    base_positions[face[0]],
                    base_positions[face[1]],
                    base_positions[face[2]],
                ]
            })
            .collect();

        for _ in 0..tessellations {
            let mut next = Vec::with_capacity(triangles.len() * 4);
            for [a, b, c] in triangles {
                let ab = mul3(add3(a, b), 0.5);
                let bc = mul3(add3(b, c), 0.5);
                let ca = mul3(add3(c, a), 0.5);
                next.push([a, ab, ca]);
                next.push([ab, b, bc]);
                next.push([ca, bc, c]);
                next.push([ab, bc, ca]);
            }
            triangles = next;
        }

        let mut vertices = Vec::with_capacity(triangles.len() * 3);
        let mut indices = Vec::with_capacity(triangles.len() * 3);

        for triangle in triangles {
            let [planar_a, planar_b, planar_c] = triangle;
            let final_positions = [planar_a, planar_b, planar_c].map(|planar| {
                let spherical = mul3(normalize3(planar), radius);
                lerp3(planar, spherical, sphericalness)
            });

            let face_normal = normalize3(cross3(
                sub3(final_positions[1], final_positions[0]),
                sub3(final_positions[2], final_positions[0]),
            ));

            for pos in final_positions {
                let spherical_normal = normalize3(pos);
                let normal = normalize3(lerp3(face_normal, spherical_normal, sphericalness));
                let u = 0.5 + normal[2].atan2(normal[0]) / std::f32::consts::TAU;
                let v = 0.5 - normal[1].asin() / std::f32::consts::PI;
                vertices.push(CpuVertex {
                    pos,
                    uv: [u, v],
                    normal,
                });
                indices.push((vertices.len() - 1) as u32);
            }
        }

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

    /// 2D partial ring/annulus in the XY plane (normal +Z).
    ///
    /// `start_angle_radians` is the arc start angle in standard polar coordinates.
    /// `sweep_angle_radians` is the angular span; negative values are accepted and
    /// are normalized to the equivalent positive arc.
    pub fn partial_annulus_2d(
        inner_radius: f32,
        outer_radius: f32,
        start_angle_radians: f32,
        sweep_angle_radians: f32,
        number_of_segments: u32,
    ) -> CpuMesh {
        let mut start = start_angle_radians;
        let mut sweep = sweep_angle_radians;
        if sweep < 0.0 {
            start += sweep;
            sweep = -sweep;
        }

        if sweep >= std::f32::consts::TAU - 1.0e-6 {
            return Self::circle_2d(inner_radius, outer_radius, number_of_segments);
        }

        let segs = number_of_segments.max(1);
        let inner = inner_radius.max(0.0);
        let outer = outer_radius.max(inner + 1.0e-6);
        let n = [0.0_f32, 0.0_f32, 1.0_f32];

        let ring_vertex_count = (segs as usize) + 1;
        let mut vertices: Vec<CpuVertex> = Vec::with_capacity(ring_vertex_count * 2);
        let mut indices: Vec<u32> = Vec::with_capacity((segs as usize) * 6);

        for i in 0..=segs {
            let t = i as f32 / segs as f32;
            let a = start + sweep * t;
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

        for i in 0..=segs {
            let t = i as f32 / segs as f32;
            let a = start + sweep * t;
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

        for i in 0..segs {
            let outer_i = i;
            let outer_n = i + 1;
            let inner_i = segs + 1 + i;
            let inner_n = inner_i + 1;

            indices.extend_from_slice(&[outer_i, outer_n, inner_n]);
            indices.extend_from_slice(&[outer_i, inner_n, inner_i]);
        }

        CpuMesh::new(vertices, indices)
    }

    /// 2D filled star centered at the origin in the XY plane (normal +Z).
    pub fn star(
        points: u32,
        inner_radius_fraction: f32,
        outer_bevel_segments: u32,
        inner_bevel_segments: u32,
    ) -> CpuMesh {
        let point_count = points.max(3);
        let outer = 0.5_f32;
        let inner = outer * inner_radius_fraction.clamp(0.01, 1.0);
        let step = std::f32::consts::PI / point_count as f32;

        let mut star_vertices: Vec<[f32; 2]> = Vec::with_capacity((point_count as usize) * 2);
        for i in 0..point_count {
            let outer_angle = i as f32 * 2.0 * step - std::f32::consts::FRAC_PI_2;
            let inner_angle = outer_angle + step;
            let (outer_s, outer_c) = outer_angle.sin_cos();
            let (inner_s, inner_c) = inner_angle.sin_cos();
            star_vertices.push([outer_c * outer, outer_s * outer]);
            star_vertices.push([inner_c * inner, inner_s * inner]);
        }

        let mut boundary: Vec<[f32; 2]> = Vec::new();
        for i in 0..star_vertices.len() {
            let prev = star_vertices[(i + star_vertices.len() - 1) % star_vertices.len()];
            let curr = star_vertices[i];
            let next = star_vertices[(i + 1) % star_vertices.len()];
            let bevel_segments = if i % 2 == 0 {
                outer_bevel_segments
            } else {
                inner_bevel_segments
            };
            append_rounded_corner_points(&mut boundary, prev, curr, next, bevel_segments, 0.35);
        }

        if signed_area_2d(&boundary) < 0.0 {
            boundary.reverse();
        }
        filled_polygon_2d(&boundary)
    }

    /// 2D filled heart centered near the origin in the XY plane (normal +Z).
    pub fn heart(number_of_segments: u32) -> CpuMesh {
        let segs = number_of_segments.max(12);
        let mut boundary: Vec<[f32; 2]> = Vec::with_capacity(segs as usize);
        for i in 0..segs {
            let t = i as f32 / segs as f32 * std::f32::consts::TAU;
            let x = 16.0 * t.sin().powi(3);
            let y =
                13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
            boundary.push([x, y]);
        }

        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for [x, y] in &boundary {
            min_x = min_x.min(*x);
            max_x = max_x.max(*x);
            min_y = min_y.min(*y);
            max_y = max_y.max(*y);
        }
        let center_x = (min_x + max_x) * 0.5;
        let center_y = (min_y + max_y) * 0.5;
        let scale = 1.0 / (max_x - min_x).max(max_y - min_y).max(1.0e-6);
        for point in &mut boundary {
            point[0] = (point[0] - center_x) * scale;
            point[1] = (point[1] - center_y) * scale;
        }

        if signed_area_2d(&boundary) < 0.0 {
            boundary.reverse();
        }
        filled_polygon_2d(&boundary)
    }
}

#[cfg(test)]
mod tests {
    use super::MeshFactory;

    fn radius2(point: [f32; 3]) -> f32 {
        (point[0] * point[0] + point[1] * point[1]).sqrt()
    }

    fn radius3(point: [f32; 3]) -> f32 {
        (point[0] * point[0] + point[1] * point[1] + point[2] * point[2]).sqrt()
    }

    #[test]
    fn sharp_star_alternates_outer_and_inner_radii() {
        let mesh = MeshFactory::star(5, 0.4, 0, 0);
        let boundary = &mesh.vertices[1..];

        assert_eq!(boundary.len(), 10);
        for (index, vertex) in boundary.iter().enumerate() {
            let expected = if index % 2 == 0 { 0.5 } else { 0.2 };
            assert!((radius2(vertex.pos) - expected).abs() < 1.0e-4);
        }
    }

    #[test]
    fn beveled_star_generates_intermediate_radii_near_tips() {
        let mesh = MeshFactory::star(5, 0.4, 3, 0);
        let boundary = &mesh.vertices[1..];

        assert!(boundary.iter().any(|vertex| {
            let r = radius2(vertex.pos);
            r > 0.2 + 1.0e-4 && r < 0.5 - 1.0e-4
        }));
    }

    #[test]
    fn icosahedron_tessellation_increases_triangle_count_by_four_per_level() {
        let base = MeshFactory::icosahedron(0, 0.0);
        let subdivided = MeshFactory::icosahedron(2, 0.0);

        assert_eq!(base.indices_u32.len() / 3, 20);
        assert_eq!(subdivided.indices_u32.len() / 3, 20 * 4 * 4);
    }

    #[test]
    fn icosahedron_sphericalness_one_projects_vertices_to_radius() {
        let mesh = MeshFactory::icosahedron(1, 1.0);

        for vertex in &mesh.vertices {
            assert!((radius3(vertex.pos) - 0.5).abs() < 1.0e-4);
        }
    }

    #[test]
    fn capsule_y_has_exact_authored_extents() {
        let radius = 0.3;
        let half_segment = 0.8;
        let mesh = MeshFactory::capsule_y(radius, half_segment, 32, 12);
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for vertex in &mesh.vertices {
            for axis in 0..3 {
                min[axis] = min[axis].min(vertex.pos[axis]);
                max[axis] = max[axis].max(vertex.pos[axis]);
            }
        }
        let expected_min = [-radius, -half_segment - radius, -radius];
        let expected_max = [radius, half_segment + radius, radius];
        for axis in 0..3 {
            assert!((min[axis] - expected_min[axis]).abs() < 1.0e-5);
            assert!((max[axis] - expected_max[axis]).abs() < 1.0e-5);
        }
    }
}
