//! Import-time height normalization for glTF assets authored in the wrong
//! units (`GLTF.default_height_if_outside(min, max, target)`). Models are
//! expected in meters; when one's rest-pose height comes out under `min` or
//! over `max` (an MMD model in ~8 cm units is about 20 m tall), one uniform
//! scale is baked into the loaded data itself (vertex positions, morph
//! position deltas, skin inverse binds, node translations at spawn) so nothing
//! downstream needs a compensating transform.
use crate::engine::graphics::mesh::CpuMesh;
use crate::engine::transform::TransformMatrix;
use crate::utils::math::{mat4_identity, mat4_mul};

/// World-space Y extent of the default scene's meshes in their rest pose, in
/// the asset's own units. Measure the imported vertices after their default
/// morphs and skin weights, without relying on any one joint's placement.
pub(super) fn rest_height<'a>(
    doc: &gltf::Document,
    inverse_binds: &[&[TransformMatrix]],
    imported_mesh: impl Fn(usize, usize) -> Option<&'a CpuMesh>,
) -> Option<f32> {
    let scene = doc.default_scene().or_else(|| doc.scenes().next())?;
    let mut worlds: Vec<Option<TransformMatrix>> = vec![None; doc.nodes().len()];
    let mut stack: Vec<(gltf::Node, TransformMatrix)> =
        scene.nodes().map(|node| (node, mat4_identity())).collect();
    while let Some((node, parent)) = stack.pop() {
        let world = mat4_mul(parent, node.transform().matrix());
        worlds[node.index()] = Some(world);
        for child in node.children() {
            stack.push((child, world));
        }
    }

    // The mesh-world transform cancels the inverse mesh-world in the skin
    // palette. Keep just the world-space Y row of each joint * inverse bind.
    let skin_rows: Vec<Vec<[f32; 4]>> = doc
        .skins()
        .zip(inverse_binds)
        .map(|(skin, binds)| {
            skin.joints()
                .zip(binds.iter())
                .map(|(joint, &bind)| {
                    let joint_world = worlds[joint.index()].unwrap_or_else(mat4_identity);
                    mat4_mul(joint_world, bind).map(|column| column[1])
                })
                .collect()
        })
        .collect();

    let mut low = f32::INFINITY;
    let mut high = f32::NEG_INFINITY;
    let mut include = |y: f32| {
        low = low.min(y);
        high = high.max(y);
    };
    for node in doc.nodes() {
        let (Some(mesh), Some(node_world)) = (node.mesh(), worlds[node.index()]) else {
            continue;
        };
        let node_row = node_world.map(|column| column[1]);
        let palette = node.skin().and_then(|skin| skin_rows.get(skin.index()));
        let weights = node.weights().or_else(|| mesh.weights()).unwrap_or(&[]);
        for primitive in mesh.primitives() {
            let Some(geometry) = imported_mesh(mesh.index(), primitive.index()) else {
                continue;
            };
            let skinning = palette
                .zip(geometry.joints0.as_deref())
                .zip(geometry.weights0.as_deref());
            let active_morphs: Vec<_> = geometry
                .morph_targets
                .iter()
                .zip(weights.iter().copied())
                .filter(|&(_, weight)| weight != 0.0)
                .collect();
            if skinning.is_none() && active_morphs.is_empty() {
                let bounds = primitive.bounding_box();
                for corner in 0..8 {
                    let position = std::array::from_fn(|axis| {
                        if corner & (1 << axis) == 0 {
                            bounds.min[axis]
                        } else {
                            bounds.max[axis]
                        }
                    });
                    include(project_y(node_row, position));
                }
                continue;
            }

            for (index, vertex) in geometry.vertices.iter().enumerate() {
                let mut position = vertex.pos;
                for &(target, weight) in &active_morphs {
                    let delta = target.position_deltas[index];
                    for axis in 0..3 {
                        position[axis] += delta[axis] * weight;
                    }
                }
                let row = if let Some(((rows, joints), weights)) = skinning {
                    skin_y_row(rows, joints[index], weights[index], node_row)?
                } else {
                    node_row
                };
                include(project_y(row, position));
            }
        }
    }
    (high > low && (high - low).is_finite()).then_some(high - low)
}

fn project_y(row: [f32; 4], [x, y, z]: [f32; 3]) -> f32 {
    row[0] * x + row[1] * y + row[2] * z + row[3]
}

fn skin_y_row(
    rows: &[[f32; 4]],
    joints: [u16; 4],
    weights: [f32; 4],
    node_row: [f32; 4],
) -> Option<[f32; 4]> {
    // The renderer uses the node transform for vertices with no skin weights.
    let weight_sum = weights.iter().sum::<f32>();
    if rows.is_empty() || weight_sum <= 0.0 {
        return Some(node_row);
    }
    let mut row = [0.0; 4];
    for (joint, weight) in joints
        .into_iter()
        .zip(weights)
        .filter(|&(_, weight)| weight != 0.0)
    {
        for (out, value) in row.iter_mut().zip(rows.get(joint as usize)?) {
            *out += value * weight;
        }
    }
    // Deformation outputs a vec3. The mesh transform applies its translation
    // once even when the input skin weights don't sum to one.
    row[3] += node_row[3] * (1.0 - weight_sum);
    Some(row)
}

/// The uniform scale to bake: `target / height` when the asset is shorter
/// than `min` or taller than `max`, otherwise 1.
pub(super) fn height_scale(height: Option<f32>, min: f32, max: f32, target: f32) -> f32 {
    match height {
        Some(height) if height > 1e-6 && (height < min || height > max) && target > 0.0 => {
            target / height
        }
        _ => 1.0,
    }
}

/// Bake a uniform scale into loaded mesh and skin data. Rotations, normals and
/// normal deltas are unchanged by a uniform scale.
pub(super) fn bake_mesh_scale(mesh: &mut CpuMesh, scale: f32) {
    for vertex in &mut mesh.vertices {
        vertex.pos = vertex.pos.map(|v| v * scale);
    }
    for target in &mut mesh.morph_targets {
        for delta in &mut target.position_deltas {
            *delta = delta.map(|v| v * scale);
        }
    }
}

/// Scaling every node translation and vertex by `scale` scales each joint's
/// rest world matrix's translation by `scale`; its inverse's translation
/// column scales the same way.
pub(super) fn bake_inverse_bind_scale(inverse_bind: &mut TransformMatrix, scale: f32) {
    for row in 0..3 {
        inverse_bind[3][row] *= scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::graphics::mesh::{CpuMorphTarget, CpuVertex};

    #[test]
    fn rest_height_uses_weighted_joints_and_default_morphs() {
        let gltf = gltf::Gltf::from_slice(
            br#"{
            "asset": {"version": "2.0"},
            "scene": 0,
            "scenes": [{"nodes": [4]}],
            "nodes": [
                {"rotation": [0.70710678, 0, 0, 0.70710678]},
                {},
                {"translation": [0, 8, 0]},
                {"mesh": 0, "skin": 0, "weights": [0.5], "translation": [0, 5, 0]},
                {"children": [0, 1, 2, 3]}
            ],
            "skins": [{"joints": [0, 1, 2]}],
            "meshes": [{"weights": [1], "primitives": [{
                "attributes": {"POSITION": 0, "JOINTS_0": 1, "WEIGHTS_0": 2},
                "targets": [{"POSITION": 3}]
            }]}],
            "buffers": [{"byteLength": 144}],
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": 36},
                {"buffer": 0, "byteOffset": 36, "byteLength": 24},
                {"buffer": 0, "byteOffset": 60, "byteLength": 48},
                {"buffer": 0, "byteOffset": 108, "byteLength": 36}
            ],
            "accessors": [
                {"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                 "min": [0, 0, 0], "max": [1, 20, 2]},
                {"bufferView": 1, "componentType": 5123, "count": 3, "type": "VEC4"},
                {"bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC4"},
                {"bufferView": 3, "componentType": 5126, "count": 3, "type": "VEC3"}
            ]
        }"#,
        )
        .unwrap();
        let vertices = [[0., 0., 0.], [0., 20., 2.], [1., 0., 1.]]
            .into_iter()
            .map(|pos| CpuVertex {
                pos,
                ..Default::default()
            })
            .collect();
        let mesh = CpuMesh::new(vertices, vec![0, 1, 2])
            .with_skinning(
                vec![[1, 0, 0, 0], [1, 2, 0, 0], [0; 4]],
                vec![[1., 0., 0., 0.], [0.5, 0.5, 0., 0.], [0.; 4]],
            )
            .with_morph_targets(vec![CpuMorphTarget {
                position_deltas: vec![[0.; 3], [0., 4., 0.], [0.; 3]],
                normal_deltas: vec![[0.; 3]; 3],
            }]);
        let binds = [mat4_identity(); 3];
        let height = rest_height(&gltf.document, &[&binds], |_, _| Some(&mesh));
        // Joint zero gives the old two-unit depth. The actual second vertex
        // gets four units from joint two and two from the node's morph weight.
        assert_eq!(height, Some(26.0));
    }

    #[test]
    fn only_assets_outside_the_range_are_scaled_to_the_target_height() {
        assert_eq!(height_scale(Some(0.2), 1.0, 3.0, 1.8), 9.0);
        assert!((height_scale(Some(20.0), 1.0, 3.0, 1.7) - 0.085).abs() < 1e-6);
        assert_eq!(height_scale(Some(1.6), 1.0, 3.0, 1.8), 1.0);
        assert_eq!(height_scale(None, 1.0, 3.0, 1.8), 1.0);
        assert_eq!(height_scale(Some(0.0), 1.0, 3.0, 1.8), 1.0);
    }

    #[test]
    fn baking_scales_translations_and_positions_but_not_rotations() {
        // A joint at (1, 2, 3) rotated a quarter turn about Z.
        let joint_world: TransformMatrix = [
            [0.0, 1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [1.0, 2.0, 3.0, 1.0],
        ];
        let mut inverse = crate::utils::math::mat4_inverse(joint_world).unwrap();
        bake_inverse_bind_scale(&mut inverse, 2.0);
        let scaled_joint = [
            joint_world[0],
            joint_world[1],
            joint_world[2],
            [2.0, 4.0, 6.0, 1.0],
        ];
        let expected = crate::utils::math::mat4_inverse(scaled_joint).unwrap();
        for col in 0..4 {
            for row in 0..4 {
                assert!((inverse[col][row] - expected[col][row]).abs() < 1e-5);
            }
        }
    }
}
