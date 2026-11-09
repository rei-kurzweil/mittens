//! Cube topology and face-local texture coordinates.
use super::{CpuMesh, CpuVertex, MeshFactory};

impl MeshFactory {
    /// Unit-ish cube centered at origin.
    ///
    /// This is a cube with per-face vertices (24 vertices, 12 triangles) so normals are flat.
    pub fn cube() -> CpuMesh {
        let p = 0.5_f32;

        // Keep each face upright with V=0 at the top, matching quad_2d.
        let mut vertices: Vec<CpuVertex> = Vec::with_capacity(24);
        let mut indices: Vec<u32> = Vec::with_capacity(36);

        let mut push_face = |n: [f32; 3], a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]| {
            let base = vertices.len() as u32;
            for pos in [a, b, c, d] {
                let uv = if n[2] != 0.0 {
                    [0.5 + n[2] * pos[0], 0.5 - pos[1]]
                } else if n[0] != 0.0 {
                    [0.5 - n[0] * pos[2], 0.5 - pos[1]]
                } else {
                    [0.5 + pos[0], 0.5 + n[1] * pos[2]]
                };
                vertices.push(CpuVertex { pos, uv, normal: n });
            }
            // CCW triangles as seen from outside.
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        };

        // -Z
        push_face(
            [0.0, 0.0, -1.0],
            [-p, -p, -p],
            [-p, p, -p],
            [p, p, -p],
            [p, -p, -p],
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_front_matches_plane_uvs_and_every_face_is_upright() {
        let cube = MeshFactory::cube();
        let plane = MeshFactory::quad_2d();
        for vertex in cube.vertices.iter().filter(|v| v.normal == [0., 0., 1.]) {
            let matching = plane
                .vertices
                .iter()
                .find(|v| v.pos[..2] == vertex.pos[..2])
                .unwrap();
            assert_eq!(vertex.uv, matching.uv);
        }
        for face in cube.vertices.chunks_exact(4) {
            let n = face[0].normal;
            let right = if n[2] != 0. {
                [n[2], 0., 0.]
            } else if n[0] != 0. {
                [0., 0., -n[0]]
            } else {
                [1., 0., 0.]
            };
            let down = if n[1] == 0. {
                [0., -1., 0.]
            } else {
                [0., 0., n[1]]
            };
            for vertex in face {
                assert_eq!(
                    vertex.uv,
                    [
                        0.5 + crate::utils::math::vec3_dot(vertex.pos, right),
                        0.5 + crate::utils::math::vec3_dot(vertex.pos, down),
                    ]
                );
            }
        }
    }
}
