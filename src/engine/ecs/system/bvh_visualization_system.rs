//! Explicitly refreshed editor snapshots. Off: no traversal, mesh generation or
//! uploads. On: draw one frozen batched wire mesh per requesting EditorUI.
use super::{
    BvhSystem, bvh_snapshot_mesh::SnapshotBoxes, static_contact_system::StaticContactSystem,
};
use crate::engine::ecs::component::EditorUIComponent;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::{
    RenderAssets, RenderUploader, VisualWorld,
    custom_material::CustomMaterialDefinition,
    materials::{MaterialBaseKind, MaterialFieldType, MaterialInputField},
    primitives::{CpuMeshHandle, GpuRenderable, InstanceHandle, Transform},
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
const STEP: Duration = Duration::from_millis(250);
#[derive(Debug, Default)]
struct Snapshot {
    revision: u64,
    enabled: bool,
    output: Option<(InstanceHandle, CpuMeshHandle)>,
    boxes: usize,
    truncated: bool,
    max_depth: u32,
    animation_step: Option<Instant>,
    active_depth: Option<u32>,
    // Cached at capture time: animation performs no per-frame allocations.
    parameters: Vec<Arc<[f32]>>,
}
impl Snapshot {
    fn animate(
        &mut self,
        owner: ComponentId,
        enabled: bool,
        now: Instant,
        visuals: &mut VisualWorld,
    ) {
        let depth = if enabled && self.output.is_some() {
            match (self.active_depth, self.animation_step) {
                (Some(depth), Some(previous)) if now.duration_since(previous) < STEP => Some(depth),
                (Some(depth), Some(_)) => {
                    // Advance only one level even after a slow frame: every
                    // level must be displayed before the traversal can restart.
                    self.animation_step = Some(now);
                    Some(if depth == self.max_depth {
                        0
                    } else {
                        depth + 1
                    })
                }
                _ => {
                    self.animation_step = Some(now);
                    Some(0)
                }
            }
        } else {
            self.animation_step = None;
            None
        };
        if self.active_depth == depth {
            return;
        }
        self.active_depth = depth;
        if let Some(definition) = visuals.materials.source_definition(owner) {
            let index = depth.map_or(0, |d| d as usize + 1);
            if let Some(parameters) = self.parameters.get(index) {
                visuals
                    .materials
                    .upsert_source(owner, definition, parameters.clone());
            }
        }
    }
}
#[derive(Debug, Default)]
pub struct BvhVisualizationSystem {
    snapshots: HashMap<ComponentId, Snapshot>,
    pub captures: u64,
}
impl BvhVisualizationSystem {
    pub fn box_count(&self, owner: ComponentId) -> usize {
        self.snapshots.get(&owner).map_or(0, |s| s.boxes)
    }
    pub fn truncated(&self, owner: ComponentId) -> bool {
        self.snapshots.get(&owner).is_some_and(|s| s.truncated)
    }
    pub fn prepare(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        assets: &mut RenderAssets,
        uploader: &mut dyn RenderUploader,
        bvh: &mut BvhSystem,
        contacts: &mut StaticContactSystem,
    ) {
        let now = Instant::now();
        self.snapshots.retain(|&owner, state| {
            if world
                .get_component_by_id_as::<EditorUIComponent>(owner)
                .is_some()
            {
                return true;
            }
            retire(state, visuals, assets, uploader);
            visuals.remove_material_source(owner);
            visuals.materials.remove_definition_source(owner);
            false
        });
        for owner in world.editor_ui_ids() {
            let ui = world
                .get_component_by_id_as::<EditorUIComponent>(owner)
                .unwrap();
            if !ui.bvh_visible {
                if let Some(state) = self.snapshots.get_mut(&owner) {
                    retire(state, visuals, assets, uploader);
                    state.enabled = false;
                    state.boxes = 0;
                }
                continue;
            }
            let state = self.snapshots.entry(owner).or_default();
            state.animate(owner, ui.bvh_animating, now, visuals);
            if state.enabled && state.revision == ui.bvh_revision {
                continue;
            }
            let mut boxes = SnapshotBoxes::default();
            // Capture only when requested, after destruction publication. Neither
            // this visual nor editor UI can enter the captured scene hierarchy.
            contacts
                .support_index(world)
                .snapshot(world, &ui.bvh_scope_roots, &mut boxes);
            bvh.flush_pending(world);
            bvh.snapshot(world, &ui.bvh_scope_roots, &mut boxes);
            boxes.limit();
            let mesh = boxes.mesh();
            let new_output = if mesh.vertices.is_empty() {
                None
            } else {
                let definition = if let Some(id) = visuals.materials.source_definition(owner) {
                    id
                } else {
                    let custom = match CustomMaterialDefinition::prepare(
                        "assets/shaders/bvh-snapshot.frag".into(),
                        ["intensity", "active_depth"]
                            .map(|name| MaterialInputField {
                                name: name.into(),
                                ty: MaterialFieldType::F32,
                            })
                            .to_vec(),
                        MaterialBaseKind::Surface,
                    ) {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("[BVH snapshot] {e}");
                            continue;
                        }
                    };
                    visuals.materials.register_custom_definition(owner, custom)
                };
                let material = visuals
                    .materials
                    .upsert_source(owner, definition, vec![1.4, -1.0]);
                let cpu = assets.register_transient_mesh(mesh);
                let gpu = match assets.gpu_mesh_handle(uploader, cpu) {
                    Ok(g) => g,
                    Err(e) => {
                        eprintln!("[BVH snapshot] upload failed: {e}");
                        assets.release_transient_mesh(uploader, cpu);
                        continue;
                    }
                };
                let handle = visuals.register(
                    owner,
                    GpuRenderable::new(
                        gpu,
                        visuals
                            .materials
                            .definition(definition)
                            .unwrap()
                            .static_program,
                    ),
                    Transform::default(),
                    [1.; 4],
                    1.,
                    false,
                    false,
                    false,
                    false,
                    false,
                    0.,
                    None,
                    3.,
                );
                visuals.bind_material_instance(handle, Some(material));
                Some((handle, cpu))
            };
            retire(state, visuals, assets, uploader);
            state.output = new_output;
            state.enabled = true;
            state.revision = ui.bvh_revision;
            state.boxes = boxes.boxes.len();
            state.truncated = boxes.truncated;
            state.max_depth = boxes
                .boxes
                .iter()
                .map(|(_, depth)| *depth)
                .max()
                .unwrap_or(0);
            state.parameters = std::iter::once(Arc::from([1.4, -1.0]))
                .chain((0..=state.max_depth).map(|depth| Arc::from([1.4, depth as f32])))
                .collect();
            state.animate(owner, ui.bvh_animating, now, visuals);
            self.captures += 1;
            eprintln!(
                "[BVH snapshot] {} boxes{}",
                state.boxes,
                if state.truncated {
                    " (snapshot limit reached)"
                } else {
                    ""
                }
            );
        }
    }
}
fn retire(
    state: &mut Snapshot,
    visuals: &mut VisualWorld,
    assets: &mut RenderAssets,
    uploader: &mut dyn RenderUploader,
) {
    state.animation_step = None;
    state.active_depth = None;
    state.parameters.clear();
    if let Some((handle, cpu)) = state.output.take() {
        visuals.remove(handle);
        assets.release_transient_mesh(uploader, cpu);
    }
}

#[cfg(test)]
#[path = "bvh_visualization_tests.rs"]
mod tests;
