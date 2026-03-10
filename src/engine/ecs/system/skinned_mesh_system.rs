use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::GLTFComponent;
use crate::engine::ecs::component::RenderableComponent;
use crate::engine::ecs::component::SkinnedMeshComponent;
use crate::engine::ecs::component::TransformComponent;
use crate::engine::ecs::system::{System, TransformSystem};
use crate::engine::graphics::SkinId;
use crate::engine::graphics::VisualWorld;
use crate::engine::graphics::primitives::TransformMatrix;
use crate::engine::user_input::InputState;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct BindingKey {
    mesh_transform: ComponentId,
    gltf_component: ComponentId,
    skin_id: SkinId,
}

/// Computes per-joint skinning matrices for skinned meshes.
///
/// This system defers to `TransformSystem` for cached world matrices.
///
/// For each `SkinnedMeshComponent`, it computes (mesh-local) skin matrices:
///
/// $$ SkinMat_j = inverse(M_meshWorld) * M_jointWorld(j) * IBM(j) $$
///
/// so the GPU can skin in mesh-local space, then apply the instance model matrix as usual.
#[derive(Debug, Default)]
pub struct SkinnedMeshSystem {
    // Reverse index so we can mark bindings dirty when a joint transform (or its ancestor) changes.
    joint_to_bindings: HashMap<ComponentId, Vec<BindingKey>>,
    // Reverse index so we can mark bindings dirty when the mesh transform (or its ancestor) changes.
    mesh_transform_to_bindings: HashMap<ComponentId, Vec<BindingKey>>,
    // Bindings that need recomputation + palette update.
    dirty_bindings: HashSet<BindingKey>,
    // Track known bindings so newly spawned rigs get computed once.
    known_bindings: HashSet<BindingKey>,

    // Per-instance joint resolution for a given (GLTFComponent instance, SkinId).
    // Stored as Option so we can keep alignment with the skin's joint order even
    // if a joint node wasn't spawned.
    instance_joints: HashMap<(ComponentId, SkinId), Vec<Option<ComponentId>>>,
}

impl SkinnedMeshSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_skin_instance_joints(
        &mut self,
        gltf_component: ComponentId,
        skin_id: SkinId,
        joints: Vec<Option<ComponentId>>,
    ) {
        self.instance_joints
            .insert((gltf_component, skin_id), joints);

        // If bindings already exist for this instance+skin, mark them dirty.
        let affected: Vec<BindingKey> = self
            .known_bindings
            .iter()
            .copied()
            .filter(|b| b.gltf_component == gltf_component && b.skin_id == skin_id)
            .collect();
        for b in affected {
            self.dirty_bindings.insert(b);
        }
    }

    /// Read-only access to the resolved joint transform ComponentIds for a particular
    /// (GLTFComponent instance, SkinId) pair.
    ///
    /// The returned slice is in the same order as `VisualWorld::skin(skin_id).joint_node_indices`.
    pub fn instance_joints_for_skin(
        &self,
        gltf_component: ComponentId,
        skin_id: SkinId,
    ) -> Option<&[Option<ComponentId>]> {
        self.instance_joints
            .get(&(gltf_component, skin_id))
            .map(|v| v.as_slice())
    }

    fn mat4_identity() -> TransformMatrix {
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    fn mat4_mul(a: TransformMatrix, b: TransformMatrix) -> TransformMatrix {
        // Column-major mat4 multiplication: out = a * b.
        let mut out = [[0.0f32; 4]; 4];
        for c in 0..4 {
            for r in 0..4 {
                out[c][r] =
                    a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2] + a[3][r] * b[c][3];
            }
        }
        out
    }

    fn update_binding(
        &self,
        world: &World,
        visuals: &VisualWorld,
        binding: BindingKey,
    ) -> Option<Vec<TransformMatrix>> {
        let mesh_world = TransformSystem::world_model(world, binding.mesh_transform)
            .unwrap_or_else(Self::mat4_identity);

        let inv_mesh_world =
            crate::utils::math::mat4_inverse(mesh_world).unwrap_or_else(Self::mat4_identity);

        let skin = visuals.skin(binding.skin_id)?;
        let joints = self
            .instance_joints
            .get(&(binding.gltf_component, binding.skin_id))?;

        let joint_count = skin.joint_count().min(joints.len());
        let mut skin_mats: Vec<TransformMatrix> = Vec::with_capacity(joint_count);

        for i in 0..joint_count {
            let joint_world = match joints[i] {
                Some(joint_cid) => TransformSystem::world_model(world, joint_cid)
                    .unwrap_or_else(Self::mat4_identity),
                None => Self::mat4_identity(),
            };
            let ibm = skin.inverse_bind_matrices[i];

            let skin_mat = Self::mat4_mul(Self::mat4_mul(inv_mesh_world, joint_world), ibm);
            skin_mats.push(skin_mat);
        }

        Some(skin_mats)
    }

    fn find_parent_renderable(world: &World, mut cid: ComponentId) -> Option<ComponentId> {
        while let Some(parent) = world.parent_of(cid) {
            if world
                .get_component_by_id_as::<RenderableComponent>(parent)
                .is_some()
            {
                return Some(parent);
            }
            cid = parent;
        }
        None
    }

    fn find_parent_transform(world: &World, mut cid: ComponentId) -> Option<ComponentId> {
        while let Some(parent) = world.parent_of(cid) {
            if world
                .get_component_by_id_as::<TransformComponent>(parent)
                .is_some()
            {
                return Some(parent);
            }
            cid = parent;
        }
        None
    }

    /// Resolve the GLTFComponent that owns the instance joints for this skin.
    ///
    /// Important: GLTFSystem spawns the node/renderable subtree under the nearest Transform
    /// ancestor (the "anchor"), and the GLTFComponent itself is typically a *child* of that
    /// anchor Transform. That means spawned nodes are often siblings (not descendants) of the
    /// GLTFComponent.
    fn find_nearest_gltf_component_for_skin(
        &self,
        world: &World,
        mut cid: ComponentId,
        skin_id: SkinId,
    ) -> Option<ComponentId> {
        let mut first_candidate: Option<ComponentId> = None;

        loop {
            // Candidate 1: the node itself.
            if world.get_component_by_id_as::<GLTFComponent>(cid).is_some() {
                if self.instance_joints.contains_key(&(cid, skin_id)) {
                    return Some(cid);
                }
                if first_candidate.is_none() {
                    first_candidate = Some(cid);
                }
            }

            // Candidate 2: any GLTFComponent child of this node.
            for &child in world.children_of(cid) {
                if world
                    .get_component_by_id_as::<GLTFComponent>(child)
                    .is_some()
                {
                    if self.instance_joints.contains_key(&(child, skin_id)) {
                        return Some(child);
                    }
                    if first_candidate.is_none() {
                        first_candidate = Some(child);
                    }
                }
            }

            let parent = world.parent_of(cid);
            match parent {
                Some(p) => cid = p,
                None => return first_candidate,
            }
        }
    }

    fn rebuild_indices(
        &mut self,
        world: &World,
        skinned: &[ComponentId],
    ) -> HashMap<BindingKey, Vec<ComponentId>> {
        self.joint_to_bindings.clear();
        self.mesh_transform_to_bindings.clear();

        let mut binding_to_renderables: HashMap<BindingKey, Vec<ComponentId>> = HashMap::new();

        for &skinned_cid in skinned {
            let Some(sm) = world.get_component_by_id_as::<SkinnedMeshComponent>(skinned_cid) else {
                continue;
            };
            let Some(skin_id) = sm.skin_id else {
                continue;
            };

            let Some(renderable_cid) = Self::find_parent_renderable(world, skinned_cid) else {
                continue;
            };

            let Some(gltf_component) =
                self.find_nearest_gltf_component_for_skin(world, skinned_cid, skin_id)
            else {
                continue;
            };

            let Some(mesh_transform) = Self::find_parent_transform(world, renderable_cid) else {
                continue;
            };

            let binding = BindingKey {
                mesh_transform,
                gltf_component,
                skin_id,
            };

            binding_to_renderables
                .entry(binding)
                .or_default()
                .push(renderable_cid);
        }

        // Build reverse index: joint -> bindings.
        for (&binding, _) in binding_to_renderables.iter() {
            self.mesh_transform_to_bindings
                .entry(binding.mesh_transform)
                .or_default()
                .push(binding);

            let Some(joints) = self
                .instance_joints
                .get(&(binding.gltf_component, binding.skin_id))
            else {
                continue;
            };

            for &joint in joints.iter().flatten() {
                self.joint_to_bindings
                    .entry(joint)
                    .or_default()
                    .push(binding);
            }
        }

        binding_to_renderables
    }

    /// Notify the system that a transform subtree changed.
    ///
    /// This walks the subtree and marks any skins referencing affected joint transforms dirty.
    pub fn transform_subtree_changed(&mut self, world: &World, root: ComponentId) {
        // Fast path: if we haven't indexed anything yet, the next tick will compute new bindings.
        if self.joint_to_bindings.is_empty() && self.mesh_transform_to_bindings.is_empty() {
            return;
        }

        let mut stack: Vec<ComponentId> = vec![root];
        while let Some(node) = stack.pop() {
            if let Some(bindings) = self.joint_to_bindings.get(&node) {
                for &binding in bindings {
                    self.dirty_bindings.insert(binding);
                }
            }

            if let Some(bindings) = self.mesh_transform_to_bindings.get(&node) {
                for &binding in bindings {
                    self.dirty_bindings.insert(binding);
                }
            }

            for &child in world.children_of(node) {
                stack.push(child);
            }
        }
    }
}

impl System for SkinnedMeshSystem {
    fn tick(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        let debug_skin_apply = std::env::var("CAT_DEBUG_SKIN_APPLY")
            .ok()
            .map(|s| {
                let s = s.trim().to_ascii_lowercase();
                s == "1" || s == "true" || s == "on" || s == "yes"
            })
            .unwrap_or(false);

        static APPLY_LOG_COUNT: AtomicUsize = AtomicUsize::new(0);

        let skinned: Vec<ComponentId> = world
            .all_components()
            .filter(|&cid| {
                world
                    .get_component_by_id_as::<SkinnedMeshComponent>(cid)
                    .is_some()
            })
            .collect();

        let binding_to_renderables = self.rebuild_indices(&*world, &skinned);

        // Mark newly discovered bindings dirty so they get an initial palette upload.
        for &binding in binding_to_renderables.keys() {
            if self.known_bindings.insert(binding) {
                self.dirty_bindings.insert(binding);
            }
        }

        // Only update bindings that are marked dirty.
        if self.dirty_bindings.is_empty() {
            return;
        }

        let dirty: Vec<BindingKey> = self.dirty_bindings.iter().copied().collect();
        self.dirty_bindings.clear();

        for binding in dirty {
            let skin_mats = match self.update_binding(&*world, visuals, binding) {
                Some(v) => v,
                None => {
                    if debug_skin_apply {
                        let n = APPLY_LOG_COUNT.fetch_add(1, Ordering::Relaxed);
                        if n < 16 {
                            let has_skin = visuals.skin(binding.skin_id).is_some();
                            let has_joints = self
                                .instance_joints
                                .contains_key(&(binding.gltf_component, binding.skin_id));
                            println!(
                                "[SkinnedMeshSystem] binding skipped: reason=missing_data has_skin={} has_instance_joints={} gltf_component={:?} mesh_transform={:?}",
                                has_skin,
                                has_joints,
                                binding.gltf_component,
                                binding.mesh_transform,
                            );
                        }
                    }
                    // If prerequisite data isn't ready yet, retry next tick.
                    self.dirty_bindings.insert(binding);
                    continue;
                }
            };

            let Some(renderables) = binding_to_renderables.get(&binding) else {
                continue;
            };

            let mut missing_handle = false;
            let mut applied = 0usize;
            let mut failed_apply = 0usize;

            for &renderable_cid in renderables {
                let Some(renderable) =
                    world.get_component_by_id_as::<RenderableComponent>(renderable_cid)
                else {
                    continue;
                };
                let Some(handle) = renderable.get_handle() else {
                    missing_handle = true;
                    continue;
                };
                let did = visuals.set_skin_matrices(handle, &skin_mats);
                if did {
                    applied += 1;
                } else {
                    failed_apply += 1;
                }
            }

            if debug_skin_apply {
                // Log a few times per run so we can see the pipeline come online.
                let n = APPLY_LOG_COUNT.fetch_add(1, Ordering::Relaxed);
                if n < 16 {
                    println!(
                        "[SkinnedMeshSystem] binding applied: skin_mats={} renderables={} applied={} failed_apply={} missing_handle={} gltf_component={:?} mesh_transform={:?}",
                        skin_mats.len(),
                        renderables.len(),
                        applied,
                        failed_apply,
                        missing_handle,
                        binding.gltf_component,
                        binding.mesh_transform,
                    );
                }
            }

            // If renderable instances aren't flushed yet, their handles will be missing here.
            // Keep the binding dirty so we retry next tick and get an initial palette upload.
            if missing_handle {
                self.dirty_bindings.insert(binding);
            }
        }
    }
}
