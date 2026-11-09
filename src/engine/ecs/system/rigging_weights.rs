//! Deliberately naive Euclidean joint weights. No surface adjacency is consulted.
use crate::engine::graphics::{CpuMesh, primitives::TransformMatrix};
use crate::utils::math::mat4_mul_vec4;

pub(super) fn assign(
    mesh: &mut CpuMesh,
    mesh_world: TransformMatrix,
    joints: &[[f32; 3]],
) -> Result<(), String> {
    if joints.is_empty() || joints.len() > u16::MAX as usize {
        return Err("automatic weights require 1..65535 joints".into());
    }
    let mut indices = Vec::with_capacity(mesh.vertices.len());
    let mut weights = Vec::with_capacity(mesh.vertices.len());
    for vertex in &mesh.vertices {
        let p = mat4_mul_vec4(
            mesh_world,
            [vertex.pos[0], vertex.pos[1], vertex.pos[2], 1.0],
        );
        let mut nearest = [(f32::INFINITY, 0u16); 4];
        for (index, joint) in joints.iter().enumerate() {
            let distance = (0..3)
                .map(|axis| (p[axis] - joint[axis]).powi(2))
                .sum::<f32>();
            if !distance.is_finite() {
                return Err("non-finite bind geometry or joint position".into());
            }
            let candidate = (distance, index as u16);
            for slot in 0..4 {
                if candidate < nearest[slot] {
                    nearest[slot..].rotate_right(1);
                    nearest[slot] = candidate;
                    break;
                }
            }
        }
        let mut ids = [0; 4];
        let mut values = [0.0; 4];
        if nearest[0].0 <= 1.0e-12 {
            ids[0] = nearest[0].1;
            values[0] = 1.0;
        } else {
            // Scale by nearest distance to avoid overflowing inverse-square scores.
            for slot in 0..4 {
                if nearest[slot].0.is_finite() {
                    ids[slot] = nearest[slot].1;
                    values[slot] = nearest[0].0 / nearest[slot].0;
                }
            }
            let total: f32 = values.iter().sum();
            for weight in &mut values {
                *weight /= total;
            }
        }
        indices.push(ids);
        weights.push(values);
    }
    mesh.joints0 = Some(indices);
    mesh.weights0 = Some(weights);
    Ok(())
}
