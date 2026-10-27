use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::RenderableComponent;
use crate::engine::ecs::system::{System, TransformSystem};
use crate::engine::graphics::SkinId;
use crate::engine::graphics::VisualWorld;
use crate::engine::graphics::primitives::TransformMatrix;
use crate::engine::user_input::InputState;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BindingKey {
    mesh_transform: ComponentId,
    gltf_component: ComponentId,
    skin_id: SkinId,
}

#[derive(Debug, Clone)]
struct SkinBinding {
    key: BindingKey,
    renderables: Vec<ComponentId>,
}

/// Computes per-joint skinning matrices for skinned meshes.
///
/// This system defers to `TransformSystem` for cached world matrices.
///
/// For each imported skin binding, it computes (mesh-local) skin matrices:
///
/// $$ SkinMat_j = inverse(M_meshWorld) * M_jointWorld(j) * IBM(j) $$
///
/// so the GPU can skin in mesh-local space, then apply the instance model matrix as usual.
#[derive(Debug, Default)]
pub struct SkinnedMeshSystem {
    // Reverse index so we can mark bindings dirty when a joint transform (or its ancestor) changes.
    joint_to_bindings: HashMap<ComponentId, Vec<usize>>,
    // Reverse index so we can mark bindings dirty when the mesh transform (or its ancestor) changes.
    mesh_transform_to_bindings: HashMap<ComponentId, Vec<usize>>,
    // Bindings that need recomputation + palette update.
    dirty_bindings: Vec<bool>,
    // Import-owned records. Normal frame iteration is over this compact list;
    // hash maps above are only reverse indexes for transform invalidation.
    bindings: Vec<Option<SkinBinding>>,

    // Per-instance joint resolution for a given (GLTFComponent instance, SkinId).
    // Stored as Option so we can keep alignment with the skin's joint order even
    // if a joint node wasn't spawned.
    instance_joints: HashMap<(ComponentId, SkinId), Vec<Option<ComponentId>>>,
    binding_profile: SkinBindingProfile,
    #[cfg(test)]
    pub(crate) profile_tick_ns: u64,
    #[cfg(test)]
    pub(crate) profile_bindings_updated: u64,
}

#[derive(Debug, Default)]
struct SkinBindingProfile {
    frames: u64,
    elapsed: Duration,
    bindings: u64,
    renderables: u64,
}

impl SkinBindingProfile {
    fn record(&mut self, elapsed: Duration, bindings: usize, renderables: usize) {
        self.frames += 1;
        self.elapsed += elapsed;
        self.bindings += bindings as u64;
        self.renderables += renderables as u64;
        if self.frames < 360 {
            return;
        }
        let frames = self.frames as f64;
        eprintln!(
            "[ImportedBindingProfile][skin] frames={} cpu_ms_per_frame={:.4} bindings_per_frame={:.1} renderables_per_frame={:.1}",
            self.frames,
            self.elapsed.as_secs_f64() * 1000.0 / frames,
            self.bindings as f64 / frames,
            self.renderables as f64 / frames,
        );
        *self = Self::default();
    }
}

fn imported_binding_profile_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CAT_PROFILE_IMPORTED_BINDINGS")
            .ok()
            .is_some_and(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "on" | "yes"
                )
            })
    })
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

        // This normally precedes binding registration. Keep the re-registration
        // path correct for asset replacement as well.
        for (index, binding) in self.bindings.iter().enumerate() {
            if binding.as_ref().is_some_and(|binding| {
                binding.key.gltf_component == gltf_component && binding.key.skin_id == skin_id
            }) {
                self.dirty_bindings[index] = true;
            }
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

    /// Register one renderable relationship discovered during glTF import.
    pub fn register_binding(
        &mut self,
        renderable: ComponentId,
        mesh_transform: ComponentId,
        gltf_component: ComponentId,
        skin_id: SkinId,
    ) {
        let key = BindingKey {
            mesh_transform,
            gltf_component,
            skin_id,
        };
        for index in 0..self.bindings.len() {
            if let Some(binding) = self.bindings[index].as_mut() {
                if binding.key == key {
                    binding.renderables.push(renderable);
                    self.dirty_bindings[index] = true;
                    return;
                }
            }
        }

        let index = self.bindings.len();
        self.bindings.push(Some(SkinBinding {
            key,
            renderables: vec![renderable],
        }));
        self.dirty_bindings.push(true);
        self.mesh_transform_to_bindings
            .entry(mesh_transform)
            .or_default()
            .push(index);
        if let Some(joints) = self.instance_joints.get(&(gltf_component, skin_id)) {
            for &joint in joints.iter().flatten() {
                self.joint_to_bindings.entry(joint).or_default().push(index);
            }
        }
    }

    /// Drop bindings as their renderables are removed. Reverse-index entries are
    /// intentionally tombstoned and ignored; their vectors are cold-path only.
    pub fn remove_renderable(&mut self, renderable: ComponentId) {
        for (index, binding) in self.bindings.iter_mut().enumerate() {
            let remove_group = if let Some(binding) = binding {
                binding
                    .renderables
                    .retain(|&candidate| candidate != renderable);
                binding.renderables.is_empty()
            } else {
                false
            };
            if remove_group {
                *binding = None;
                self.dirty_bindings[index] = false;
            }
        }
    }

    pub fn skin_id_for_renderable(&self, renderable: ComponentId) -> Option<SkinId> {
        self.bindings.iter().flatten().find_map(|binding| {
            binding
                .renderables
                .contains(&renderable)
                .then_some(binding.key.skin_id)
        })
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
                for &index in bindings {
                    if self.bindings.get(index).is_some_and(Option::is_some) {
                        self.dirty_bindings[index] = true;
                    }
                }
            }

            if let Some(bindings) = self.mesh_transform_to_bindings.get(&node) {
                for &index in bindings {
                    if self.bindings.get(index).is_some_and(Option::is_some) {
                        self.dirty_bindings[index] = true;
                    }
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
        #[cfg(test)]
        let profile_tick_started = Instant::now();
        let profile = imported_binding_profile_enabled();
        let debug_skin_apply = std::env::var("CAT_DEBUG_SKIN_APPLY")
            .ok()
            .map(|s| {
                let s = s.trim().to_ascii_lowercase();
                s == "1" || s == "true" || s == "on" || s == "yes"
            })
            .unwrap_or(false);

        static APPLY_LOG_COUNT: AtomicUsize = AtomicUsize::new(0);

        let started = profile.then(Instant::now);
        let binding_count = self.bindings.iter().flatten().count();
        let renderable_count = self
            .bindings
            .iter()
            .flatten()
            .map(|binding| binding.renderables.len())
            .sum();

        for index in 0..self.bindings.len() {
            if !self.dirty_bindings[index] {
                continue;
            }
            self.dirty_bindings[index] = false;
            #[cfg(test)]
            {
                self.profile_bindings_updated += 1;
            }
            let Some(binding) = self.bindings[index].clone() else {
                continue;
            };

            let skin_mats = match self.update_binding(&*world, visuals, binding.key) {
                Some(v) => v,
                None => {
                    if debug_skin_apply {
                        let n = APPLY_LOG_COUNT.fetch_add(1, Ordering::Relaxed);
                        if n < 16 {
                            let has_skin = visuals.skin(binding.key.skin_id).is_some();
                            let has_joints = self
                                .instance_joints
                                .contains_key(&(binding.key.gltf_component, binding.key.skin_id));
                            println!(
                                "[SkinnedMeshSystem] binding skipped: reason=missing_data has_skin={} has_instance_joints={} gltf_component={:?} mesh_transform={:?}",
                                has_skin,
                                has_joints,
                                binding.key.gltf_component,
                                binding.key.mesh_transform,
                            );
                        }
                    }
                    // If prerequisite data isn't ready yet, retry next tick.
                    self.dirty_bindings[index] = true;
                    continue;
                }
            };

            let mut missing_handle = false;
            let mut applied = 0usize;
            let mut failed_apply = 0usize;

            for &renderable_cid in &binding.renderables {
                let Some(renderable) =
                    world.get_component_by_id_as::<RenderableComponent>(renderable_cid)
                else {
                    continue;
                };
                let Some(handle) = renderable.get_handle() else {
                    missing_handle = true;
                    continue;
                };
                if visuals.set_skin_matrices(handle, &skin_mats) {
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
                        binding.renderables.len(),
                        applied,
                        failed_apply,
                        missing_handle,
                        binding.key.gltf_component,
                        binding.key.mesh_transform,
                    );
                }
            }

            // If renderable instances aren't flushed yet, their handles will be missing here.
            // Keep the binding dirty so we retry next tick and get an initial palette upload.
            if missing_handle {
                self.dirty_bindings[index] = true;
            }
        }

        #[cfg(test)]
        {
            self.profile_tick_ns += profile_tick_started.elapsed().as_nanos() as u64;
        }
        if let Some(started) = started {
            self.binding_profile
                .record(started.elapsed(), binding_count, renderable_count);
        }
    }
}
