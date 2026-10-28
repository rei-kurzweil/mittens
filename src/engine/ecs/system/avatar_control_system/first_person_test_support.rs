//! CPU-skinned surface probes shared by real first-person regression tests.
use super::*;

pub(crate) fn body_hit(
    document: &gltf::Gltf,
    world: &World,
    nodes: &[ComponentId],
    view: [[f32; 4]; 4],
    ray: [f32; 3],
) -> Option<(f32, f32)> {
    use crate::utils::math::{vec3_cross, vec3_dot, vec3_sub};
    let origin = view[3][..3].try_into().unwrap();
    let direction4 = mat4_mul_vec4(view, [ray[0], ray[1], ray[2], 0.0]);
    let direction: [f32; 3] = direction4[..3].try_into().unwrap();
    let mut nearest: Option<(f32, f32)> = None;
    for node in document.nodes() {
        let Some(mesh) = node
            .mesh()
            .filter(|m| m.name().is_some_and(|n| n.starts_with("Body")))
        else {
            continue;
        };
        let skin = node.skin().unwrap();
        let inverse: Vec<_> = skin
            .reader(|_| document.blob.as_deref())
            .read_inverse_bind_matrices()
            .unwrap()
            .collect();
        let joints: Vec<_> = skin
            .joints()
            .enumerate()
            .map(|(i, joint)| {
                mat4_mul(
                    world
                        .get_component_by_id_as::<TransformComponent>(nodes[joint.index()])
                        .unwrap()
                        .transform
                        .matrix_world,
                    inverse[i],
                )
            })
            .collect();
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|_| document.blob.as_deref());
            let positions = reader.read_positions().unwrap();
            let joint_ids = reader.read_joints(0).unwrap().into_u16();
            let weights = reader.read_weights(0).unwrap().into_f32();
            let vertices: Vec<[f32; 3]> = positions
                .zip(joint_ids)
                .zip(weights)
                .map(|((p, ids), weights)| {
                    let mut skinned = [0.0; 3];
                    for i in 0..4 {
                        let p = mat4_mul_vec4(joints[ids[i] as usize], [p[0], p[1], p[2], 1.0]);
                        for axis in 0..3 {
                            skinned[axis] += weights[i] * p[axis];
                        }
                    }
                    skinned
                })
                .collect();
            let indices: Vec<_> = reader.read_indices().unwrap().into_u32().collect();
            for triangle in indices.chunks_exact(3) {
                let a = vertices[triangle[0] as usize];
                let e1 = vec3_sub(vertices[triangle[1] as usize], a);
                let e2 = vec3_sub(vertices[triangle[2] as usize], a);
                let cross = vec3_cross(direction, e2);
                let det = vec3_dot(e1, cross);
                if det.abs() < 1e-9 {
                    continue;
                }
                let offset = vec3_sub(origin, a);
                let u = vec3_dot(offset, cross) / det;
                let v = vec3_dot(direction, vec3_cross(offset, e1)) / det;
                if u < 0.0 || v < 0.0 || u + v > 1.0 {
                    continue;
                }
                let distance = vec3_dot(e2, vec3_cross(offset, e1)) / det;
                if distance > 0.0001 && nearest.is_none_or(|hit| distance < hit.0) {
                    nearest = Some((distance, vec3_dot(vec3_cross(e1, e2), direction)));
                }
            }
        }
    }
    nearest
}
