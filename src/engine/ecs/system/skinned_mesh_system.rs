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

    fn find_parent_gltf_component(world: &World, mut cid: ComponentId) -> Option<ComponentId> {
        loop {
            if world.get_component_by_id_as::<GLTFComponent>(cid).is_some() {
                return Some(cid);
            }
            let parent = world.parent_of(cid)?;
            cid = parent;
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

            let Some(gltf_component) = Self::find_parent_gltf_component(world, skinned_cid) else {
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
            let Some(skin_mats) = self.update_binding(&*world, visuals, binding) else {
                continue;
            };

            let Some(renderables) = binding_to_renderables.get(&binding) else {
                continue;
            };

            for &renderable_cid in renderables {
                let Some(renderable) =
                    world.get_component_by_id_as::<RenderableComponent>(renderable_cid)
                else {
                    continue;
                };
                let Some(handle) = renderable.get_handle() else {
                    continue;
                };
                let _ = visuals.set_skin_matrices(handle, &skin_mats);
            }
        }
    }
}
