//! Import-time height normalization for glTF assets authored in the wrong
//! units (`GLTF.default_height_if_short(min, target)`). Models are expected in
//! meters; when one's rest-pose height comes out shorter than `min`, one
//! uniform scale is baked into the loaded data itself (vertex positions, morph
//! position deltas, skin inverse binds, node translations at spawn) so nothing
//! downstream needs a compensating transform.
use crate::engine::graphics::mesh::CpuMesh;
use crate::engine::transform::TransformMatrix;
use crate::utils::math::{mat4_mul, mat4_mul_vec4};

/// World-space Y extent of the default scene's meshes in their rest pose, in
/// the asset's own units. Skinned meshes are placed by their bind transform
/// (first joint's rest world matrix times its inverse bind), not by the raw
/// vertex box, so meshes stored sideways and stood up by the skeleton measure
/// upright. None when the scene has no measurable mesh.
pub(super) fn rest_height(
    doc: &gltf::Document,
    inverse_binds: &[Vec<TransformMatrix>],
) -> Option<f32> {
    let scene = doc.default_scene().or_else(|| doc.scenes().next())?;
    let mut worlds: Vec<Option<TransformMatrix>> = vec![None; doc.nodes().len()];
    let mut stack: Vec<(gltf::Node, TransformMatrix)> = scene
        .nodes()
        .map(|node| (node, IDENTITY))
        .collect();
    while let Some((node, parent)) = stack.pop() {
        let world = mat4_mul(parent, node.transform().matrix());
        worlds[node.index()] = Some(world);
        for child in node.children() {
            stack.push((child, world));
        }
    }
    let mut low = f32::INFINITY;
    let mut high = f32::NEG_INFINITY;
    for node in doc.nodes() {
        let (Some(mesh), Some(node_world)) = (node.mesh(), worlds[node.index()]) else {
            continue;
        };
        let placement = node
            .skin()
            .and_then(|skin| {
                let joint = skin.joints().next()?;
                let joint_world = worlds[joint.index()]?;
                let inverse_bind = inverse_binds.get(skin.index())?.first()?;
                Some(mat4_mul(joint_world, *inverse_bind))
            })
            .unwrap_or(node_world);
        for primitive in mesh.primitives() {
            let bounds = primitive.bounding_box();
            for corner in 0..8 {
                let pick = |axis: usize| {
                    if corner & (1 << axis) == 0 {
                        bounds.min[axis]
                    } else {
                        bounds.max[axis]
                    }
                };
                let y = mat4_mul_vec4(placement, [pick(0), pick(1), pick(2), 1.0])[1];
                low = low.min(y);
                high = high.max(y);
            }
        }
    }
    (high > low && (high - low).is_finite()).then_some(high - low)
}

/// The uniform scale to bake: `target / height` when the asset is shorter
/// than `min`, otherwise 1.
pub(super) fn height_scale(height: Option<f32>, min: f32, target: f32) -> f32 {
    match height {
        Some(height) if height > 1e-6 && height < min && target > 0.0 => target / height,
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

const IDENTITY: TransformMatrix = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_short_assets_are_scaled_to_the_target_height() {
        assert_eq!(height_scale(Some(0.2), 1.0, 1.8), 9.0);
        assert_eq!(height_scale(Some(1.6), 1.0, 1.8), 1.0);
        assert_eq!(height_scale(None, 1.0, 1.8), 1.0);
        assert_eq!(height_scale(Some(0.0), 1.0, 1.8), 1.0);
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
