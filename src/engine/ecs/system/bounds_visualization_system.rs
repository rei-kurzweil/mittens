use std::collections::{HashMap, HashSet};

use crate::engine::ecs::component::EditorComponent;
use crate::engine::ecs::system::{
    GLTFSystem, GltfBoundsVisualizationSystem, MeshBoundsSystem, MeshOutputKind,
};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::{RenderAssets, VisualWorld};

/// Editor-facing mesh-bounds overlay coordinator.
///
/// GLTF discovery remains in `GltfBoundsVisualizationSystem`; generated and
/// future generic outputs are discovered through `MeshBoundsSystem` here.
#[derive(Debug, Default)]
pub struct BoundsVisualizationSystem {
    pub(crate) bvh: super::bvh_visualization_system::BvhVisualizationSystem,
    mesh_output_markers: HashMap<ComponentId, ComponentId>,
}

impl BoundsVisualizationSystem {
    #[allow(clippy::too_many_arguments)]
    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        gltf_bounds: &mut GltfBoundsVisualizationSystem,
        gltf_system: &GLTFSystem,
        mesh_bounds: &MeshBoundsSystem,
        mesh_bounds_visible: bool,
        visuals: &mut VisualWorld,
        render_assets: &mut RenderAssets,
        emit: &mut dyn SignalEmitter,
    ) {
        gltf_bounds.tick_with_queue(world, gltf_system, visuals, render_assets, emit);
        self.sync_mesh_output_markers(world, render_assets, emit, mesh_bounds, mesh_bounds_visible);
    }

    fn sync_mesh_output_markers(
        &mut self,
        world: &mut World,
        render_assets: &mut RenderAssets,
        emit: &mut dyn SignalEmitter,
        mesh_bounds: &MeshBoundsSystem,
        visible: bool,
    ) {
        let outputs: Vec<_> = mesh_bounds
            .outputs()
            .filter(|output| {
                output.kind == MeshOutputKind::CombineMesh
                    && is_within_editor_scope(world, output.owner)
            })
            .collect();
        let live_owners: HashSet<_> = outputs.iter().map(|output| output.owner).collect();

        let stale: Vec<_> = self
            .mesh_output_markers
            .iter()
            .filter_map(|(&owner, &marker_root)| {
                (!live_owners.contains(&owner)
                    || world.get_component_record(marker_root).is_none()
                    || world.get_component_record(owner).is_none())
                .then_some((owner, marker_root))
            })
            .collect();
        for (owner, marker_root) in stale {
            self.mesh_output_markers.remove(&owner);
            emit.push_intent_now(
                marker_root,
                IntentValue::RemoveSubtree {
                    component_id: marker_root,
                },
            );
        }

        if !visible {
            for &marker_root in self.mesh_output_markers.values() {
                emit.push_intent_now(
                    marker_root,
                    IntentValue::RemoveSubtree {
                        component_id: marker_root,
                    },
                );
            }
            self.mesh_output_markers.clear();
            return;
        }

        for output in outputs {
            if self.mesh_output_markers.contains_key(&output.owner)
                || world.get_component_record(output.owner).is_none()
            {
                continue;
            }
            let marker_root =
                crate::engine::ecs::system::gltf_bounds_visualization_system::spawn_bounds_marker(
                    world,
                    render_assets,
                    emit,
                    output.owner,
                    output.local,
                );
            self.mesh_output_markers.insert(output.owner, marker_root);
        }
    }
}

fn is_within_editor_scope(world: &World, owner: ComponentId) -> bool {
    let mut current = Some(owner);
    while let Some(component) = current {
        if world
            .get_component_by_id_as::<EditorComponent>(component)
            .is_some()
        {
            return true;
        }
        current = world.parent_of(component);
    }
    false
}
