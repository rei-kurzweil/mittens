#[path = "secondary_motion_binding.rs"]
mod binding;
#[path = "secondary_motion_rest_targets.rs"]
mod rest_targets;
#[path = "secondary_motion_stretch.rs"]
mod stretch;
use super::collidable_bvh::CollidableBvh;
use crate::engine::ecs::component::CollisionShape;
use crate::engine::ecs::component::{
    ArmatureComponent, BoneRestPoseComponent, ComponentRef, GLTFComponent, QueryRootMode,
    SecondaryMotionComponent, SpringBoneComponent, SpringColliderComponent, SpringJointComponent,
    TransformComponent, resolve_component_ref,
};
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, RxWorld, Signal, SignalEmitter, SignalKind, World,
};
use crate::utils::math::{
    mat_to_quat, quat_conjugate, quat_mul, quat_normalize, quat_rotate_vec3, shortest_arc_quat,
    vec3_add, vec3_len, vec3_normalize, vec3_scale, vec3_sub,
};
use binding::*;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

const STEP: f32 = 1.0 / 60.0;

#[derive(Debug, Clone)]
struct JointConfig {
    id: ComponentId,
    parent_id: ComponentId,
    rest_rotation: [f32; 4],
    rest_direction: [f32; 3],
    stiffness: f32,
    drag: f32,
    gravity: [f32; 3],
}

#[derive(Debug, Clone)]
struct BoundChain {
    joints: Vec<JointConfig>,
    rest_targets: Option<rest_targets::RestTargets>,
    allow_stretch: bool,
    previous: Vec<[f32; 3]>,
    current: Vec<[f32; 3]>,
    lengths: Vec<f32>,
    accumulator: f32,
    enabled: bool,
    parked: bool,
    armature_owner: ComponentId,
    hit_radius: f32,
    colliders: Vec<BoundCollider>,
}

#[derive(Debug, Clone)]
struct BoundCollider {
    config: ComponentId,
    target: ComponentId,
    radius: f32,
}

/// Immutable primary-pose sample, shared by every chain and fixed substep.
#[derive(Debug, Clone, Copy)]
struct WorldSphere {
    armature_owner: ComponentId,
    center: [f32; 3],
    radius: f32,
}

#[derive(Default)]
struct CollisionFrame {
    by_source: HashMap<(ComponentId, ComponentId), WorldSphere>,
    by_x: Vec<WorldSphere>,
    max_radius: f32,
}

struct ChainContacts<'a> {
    spheres: Vec<WorldSphere>,
    surfaces: Option<&'a CollidableBvh>,
    surface_ids: Vec<usize>,
    hit_radius: f32,
}

#[derive(Debug, Clone)]
enum ChainStatus {
    WaitingForDependencies(String),
    Bound(BoundChain),
    Invalid(String),
}

impl Default for ChainStatus {
    fn default() -> Self {
        Self::WaitingForDependencies("not bound yet".into())
    }
}

#[derive(Debug, Default)]
struct RootRuntime {
    armature_owner: Option<ComponentId>,
    children: HashSet<ComponentId>,
}

#[derive(Debug, Default)]
struct ChainRuntime {
    root: Option<ComponentId>,
    joint_config_ids: Vec<ComponentId>,
    resolved_ids: Vec<ComponentId>,
    status: ChainStatus,
}

#[derive(Debug, Default, Clone, Copy)]
struct Profile {
    binding: Duration,
    invalidation: Duration,
    simulation: Duration,
    binding_attempts: u64,
    invalidations: u64,
    topology_discoveries: u64,
    selector_resolutions: u64,
}

#[derive(Debug, Default)]
pub struct SecondaryMotionSystem {
    paused: bool,
    collider_pool_revision: Option<(u64, u64)>,
    collider_pool: Vec<(ComponentId, BoundCollider)>,
    registered: HashSet<ComponentId>,
    roots: HashMap<ComponentId, RootRuntime>,
    chains: HashMap<ComponentId, ChainRuntime>,
    child_owner: HashMap<ComponentId, ComponentId>,
    joint_owner: HashMap<ComponentId, ComponentId>,
    owner_roots: HashMap<ComponentId, HashSet<ComponentId>>,
    resolved_transform_chains: HashMap<ComponentId, HashSet<ComponentId>>,
    joint_claims: HashMap<ComponentId, ComponentId>,
    diagnostics: HashMap<ComponentId, String>,
    collider_diagnostics: HashSet<String>,
    profile: Profile,
    debug_frames: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SecondaryMotionColliderSnapshot {
    pub config: ComponentId,
    pub target: ComponentId,
    pub center: [f32; 3],
    pub scaled_base_radius: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SecondaryMotionSegmentSnapshot {
    pub head: [f32; 3],
    pub end: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct SecondaryMotionChainSnapshot {
    pub armature_owner: ComponentId,
    pub chain: ComponentId,
    pub enabled: bool,
    pub hit_radius: f32,
    pub scaled_hit_radius: f32,
    pub segments: Vec<SecondaryMotionSegmentSnapshot>,
    pub colliders: Vec<SecondaryMotionColliderSnapshot>,
}

impl SecondaryMotionSystem {
    /// Resolved first-joint parent and armature owner for constraint sampling.
    pub fn chain_anchor(&self, chain: ComponentId) -> Option<(ComponentId, ComponentId)> {
        let ChainStatus::Bound(bound) = &self.chains.get(&chain)?.status else {
            return None;
        };
        Some((bound.joints.first()?.parent_id, bound.armature_owner))
    }
    /// Read-only projection of successfully bound solver state in current world space.
    pub fn bound_snapshot(&self, world: &World) -> Vec<SecondaryMotionChainSnapshot> {
        let mut snapshot = Vec::new();
        for (&chain, runtime) in &self.chains {
            let ChainStatus::Bound(state) = &runtime.status else {
                continue;
            };
            let hit_scale = owner_anchor(world, state.armature_owner)
                .map(|anchor| world_max_scale(world, anchor))
                .unwrap_or(1.0);
            let segments = state
                .joints
                .iter()
                .enumerate()
                .map(|(index, joint)| SecondaryMotionSegmentSnapshot {
                    head: if index == 0 {
                        pos(world, joint.id).unwrap_or(state.current[index])
                    } else {
                        state.current[index - 1]
                    },
                    end: state.current[index],
                })
                .collect();
            let colliders = state
                .colliders
                .iter()
                .filter_map(|collider| {
                    Some(SecondaryMotionColliderSnapshot {
                        config: collider.config,
                        target: collider.target,
                        center: pos(world, collider.target)?,
                        scaled_base_radius: collider.radius
                            * world_max_scale(world, collider.target),
                    })
                })
                .collect();
            snapshot.push(SecondaryMotionChainSnapshot {
                armature_owner: state.armature_owner,
                chain,
                enabled: state.enabled,
                hit_radius: state.hit_radius,
                scaled_hit_radius: state.hit_radius * hit_scale,
                segments,
                colliders,
            });
        }
        snapshot.sort_by_key(|entry| entry.chain);
        snapshot
    }

    pub fn install_handlers(rx: &mut RxWorld) {
        rx.add_global_handler_named(
            SignalKind::ParentChanged,
            Some("secondary_motion_topology".into()),
            secondary_motion_parent_changed,
        );
        rx.add_global_handler_named(
            SignalKind::GltfInitialized,
            Some("secondary_motion_gltf_initialized".into()),
            secondary_motion_gltf_initialized,
        );
    }

    /// Idempotently registers a root, supported simulation child, or joint configuration.
    pub fn register(&mut self, world: &World, component: ComponentId) {
        if !self.registered.insert(component) {
            return;
        }
        if world
            .get_component_by_id_as::<SecondaryMotionComponent>(component)
            .is_some()
        {
            self.roots.entry(component).or_default();
            self.reconcile_root(world, component);
            self.profile.topology_discoveries += 1;
            let children: Vec<_> = world.children_of(component).to_vec();
            for child in children {
                if world
                    .get_component_by_id_as::<SpringBoneComponent>(child)
                    .is_some()
                {
                    self.register(world, child);
                }
            }
            return;
        }
        if world
            .get_component_by_id_as::<SpringBoneComponent>(component)
            .is_some()
        {
            self.chains.entry(component).or_default();
            self.profile.topology_discoveries += 1;
            if let Some(parent) = world.parent_of(component)
                && world
                    .get_component_by_id_as::<SecondaryMotionComponent>(parent)
                    .is_some()
                && !self.registered.contains(&parent)
            {
                self.register(world, parent);
                if self.child_owner.get(&component) == Some(&parent) {
                    return;
                }
            }
            self.reconcile_chain(world, component);
            return;
        }
        if world
            .get_component_by_id_as::<SpringJointComponent>(component)
            .is_some()
        {
            self.profile.topology_discoveries += 1;
            if let Some(chain) = world.parent_of(component)
                && world
                    .get_component_by_id_as::<SpringBoneComponent>(chain)
                    .is_some()
            {
                if !self.registered.contains(&chain) {
                    self.register(world, chain);
                } else {
                    self.reconcile_chain(world, chain);
                }
            }
            return;
        }
        if world
            .get_component_by_id_as::<SpringColliderComponent>(component)
            .is_some()
        {
            let owning_gltf = nearest_armature_owner(world, component);
            let chains: Vec<_> = self
                .chains
                .iter()
                .filter_map(|(&chain, runtime)| {
                    let root = runtime.root?;
                    (self.roots.get(&root)?.armature_owner == owning_gltf
                        && world
                            .get_component_by_id_as::<SpringBoneComponent>(chain)
                            .is_some_and(|chain| !chain.colliders.is_empty()))
                    .then_some(chain)
                })
                .collect();
            for chain in chains {
                self.bind_chain(world, chain);
            }
        }
    }

    pub fn topology_changed(&mut self, world: &World, component: ComponentId) {
        let started = Instant::now();
        let binding_before = self.profile.binding;
        self.profile.invalidations += 1;
        let mut chains = HashSet::new();
        if world
            .get_component_by_id_as::<SpringColliderComponent>(component)
            .is_some()
        {
            let owning_gltf = nearest_armature_owner(world, component);
            for (&chain, runtime) in &self.chains {
                let same_gltf = runtime
                    .root
                    .and_then(|root| self.roots.get(&root))
                    .and_then(|root| root.armature_owner)
                    == owning_gltf;
                if same_gltf
                    && world
                        .get_component_by_id_as::<SpringBoneComponent>(chain)
                        .is_some_and(|chain| !chain.colliders.is_empty())
                {
                    chains.insert(chain);
                }
            }
        }
        for (&chain, runtime) in &self.chains {
            if let ChainStatus::Bound(bound) = &runtime.status
                && bound
                    .colliders
                    .iter()
                    .any(|collider| collider.config == component || collider.target == component)
            {
                chains.insert(chain);
            }
        }
        if self.roots.contains_key(&component) {
            self.reconcile_root(world, component);
            chains.extend(self.roots[&component].children.iter().copied());
        }
        if self.chains.contains_key(&component) {
            self.reconcile_chain(world, component);
        }
        if let Some(chain) = self.joint_owner.get(&component).copied() {
            chains.insert(chain);
        }
        if self.registered.contains(&component)
            && world
                .get_component_by_id_as::<SpringJointComponent>(component)
                .is_some()
            && let Some(chain) = world.parent_of(component)
            && self.chains.contains_key(&chain)
        {
            chains.insert(chain);
        }
        if let Some(owners) = self.resolved_transform_chains.get(&component) {
            chains.extend(owners.iter().copied());
        }
        // A newly attached/detached skin joint is not in the previous resolved
        // list yet. Walking its ancestors makes automatic chains re-discover
        // topology after ParentChanged events.
        let mut ancestor = world.parent_of(component);
        for _ in 0..64 {
            let Some(id) = ancestor else { break };
            if let Some(owners) = self.resolved_transform_chains.get(&id) {
                chains.extend(owners.iter().copied());
            }
            ancestor = world.parent_of(id);
        }
        if let Some(roots) = self.owner_roots.get(&component) {
            for root in roots {
                chains.extend(self.roots[root].children.iter().copied());
            }
        }
        // Automatic chains depend on the imported skin topology, including
        // children which may not have belonged to the last successful bind.
        // ParentChanged also schedules this hook for old/new parents, so a
        // detached branch can recover an invalid chain.
        for (&candidate, runtime) in &self.chains {
            let automatic = runtime.joint_config_ids.is_empty()
                && world
                    .get_component_by_id_as::<SpringBoneComponent>(candidate)
                    .is_some_and(|chain| chain.root.is_some());
            if !automatic {
                continue;
            }
            let Some(armature_owner) = runtime
                .root
                .and_then(|root| self.roots.get(&root))
                .and_then(|root| root.armature_owner)
            else {
                continue;
            };
            if skin_joints(world, armature_owner).contains(&component) {
                chains.insert(candidate);
            }
        }
        for chain in chains {
            self.reconcile_chain(world, chain);
        }
        self.profile.invalidation += started
            .elapsed()
            .saturating_sub(self.profile.binding.saturating_sub(binding_before));
    }

    pub fn configuration_changed(&mut self, world: &World, component: ComponentId) {
        let started = Instant::now();
        let binding_before = self.profile.binding;
        self.profile.invalidations += 1;
        let collider_chains: Vec<_> = self
            .chains
            .iter()
            .filter_map(|(&chain, runtime)| match &runtime.status {
                ChainStatus::Bound(bound)
                    if bound
                        .colliders
                        .iter()
                        .any(|collider| collider.config == component) =>
                {
                    Some(chain)
                }
                _ => None,
            })
            .collect();
        if self.chains.contains_key(&component) {
            self.reconcile_chain(world, component);
        } else if let Some(chain) = self.joint_owner.get(&component).copied() {
            self.reconcile_chain(world, chain);
        } else if let Some(root) = self.roots.get(&component) {
            let chains: Vec<_> = root.children.iter().copied().collect();
            for chain in chains {
                self.bind_chain(world, chain);
            }
        }
        for chain in collider_chains {
            self.bind_chain(world, chain);
        }
        self.profile.invalidation += started
            .elapsed()
            .saturating_sub(self.profile.binding.saturating_sub(binding_before));
    }

    pub fn gltf_initialized(&mut self, world: &World, armature_owner: ComponentId) {
        self.armature_initialized(world, armature_owner);
    }

    pub fn armature_initialized(&mut self, world: &World, armature_owner: ComponentId) {
        let roots: Vec<_> = self
            .owner_roots
            .get(&armature_owner)
            .into_iter()
            .flatten()
            .copied()
            .collect();
        for root in roots {
            let chains: Vec<_> = self.roots[&root].children.iter().copied().collect();
            for chain in chains {
                self.bind_chain(world, chain);
            }
        }
    }

    /// Cleanup hook used by subtree removal before graph records disappear.
    pub fn component_removed(&mut self, world: &World, component: ComponentId) {
        let collider_chains: Vec<_> = self
            .chains
            .iter()
            .filter_map(|(&chain, runtime)| match &runtime.status {
                ChainStatus::Bound(bound)
                    if bound.colliders.iter().any(|collider| {
                        collider.config == component || collider.target == component
                    }) =>
                {
                    Some(chain)
                }
                _ => None,
            })
            .collect();
        self.registered.remove(&component);
        if self.roots.contains_key(&component) {
            self.remove_root(component);
        }
        if self.chains.contains_key(&component) {
            let root = self.child_owner.get(&component).copied();
            self.remove_chain(component);
            if let Some(root) = root {
                let retry: Vec<_> = self.roots[&root]
                    .children
                    .iter()
                    .copied()
                    .filter(|chain| matches!(self.chains[chain].status, ChainStatus::Invalid(_)))
                    .collect();
                for chain in retry {
                    self.bind_chain(world, chain);
                }
            }
        }
        if let Some(chain) = self.joint_owner.remove(&component) {
            if let Some(runtime) = self.chains.get_mut(&chain) {
                runtime.joint_config_ids.retain(|id| *id != component);
                runtime.status =
                    ChainStatus::WaitingForDependencies("joint configuration was removed".into());
            }
            self.release_resolved_ownership(chain);
            self.bind_chain(world, chain);
        }
        if let Some(chains) = self.resolved_transform_chains.remove(&component) {
            for chain in chains {
                self.release_resolved_ownership(chain);
                if let Some(runtime) = self.chains.get_mut(&chain) {
                    runtime.status = ChainStatus::WaitingForDependencies(
                        "resolved imported transform was removed".into(),
                    );
                }
            }
        }
        if let Some(roots) = self.owner_roots.remove(&component) {
            for root in roots {
                if let Some(runtime) = self.roots.get_mut(&root) {
                    runtime.armature_owner = None;
                    let children: Vec<_> = runtime.children.iter().copied().collect();
                    for chain in children {
                        self.release_resolved_ownership(chain);
                        self.chains.get_mut(&chain).unwrap().status =
                            ChainStatus::WaitingForDependencies("owning GLTF was removed".into());
                    }
                }
            }
        }
        self.diagnostics.remove(&component);
        for chain in collider_chains {
            if self.chains.contains_key(&chain) {
                self.bind_chain(world, chain);
            }
        }
    }

    pub fn reset(&mut self, world: &World, target: ComponentId) {
        let mut chains = HashSet::new();
        if self.chains.contains_key(&target) {
            chains.insert(target);
        }
        if let Some(root) = self.roots.get(&target) {
            chains.extend(root.children.iter().copied());
        }
        if let Some(roots) = self.owner_roots.get(&target) {
            for root in roots {
                chains.extend(self.roots[root].children.iter().copied());
            }
        }
        for chain in chains {
            self.bind_chain(world, chain);
        }
    }

    fn remove_root(&mut self, root: ComponentId) {
        let Some(runtime) = self.roots.remove(&root) else {
            return;
        };
        if let Some(armature_owner) = runtime.armature_owner
            && let Some(roots) = self.owner_roots.get_mut(&armature_owner)
        {
            roots.remove(&root);
            if roots.is_empty() {
                self.owner_roots.remove(&armature_owner);
            }
        }
        for chain in runtime.children {
            self.child_owner.remove(&chain);
            self.release_resolved_ownership(chain);
            if let Some(runtime) = self.chains.get_mut(&chain) {
                runtime.root = None;
                runtime.status = ChainStatus::Invalid(
                    "SpringBone must be a direct child of SecondaryMotion".into(),
                );
            }
        }
    }

    fn remove_chain(&mut self, chain: ComponentId) {
        self.release_resolved_ownership(chain);
        if let Some(root) = self.child_owner.remove(&chain)
            && let Some(runtime) = self.roots.get_mut(&root)
        {
            runtime.children.remove(&chain);
        }
        if let Some(runtime) = self.chains.remove(&chain) {
            for joint in runtime.joint_config_ids {
                if self.joint_owner.get(&joint) == Some(&chain) {
                    self.joint_owner.remove(&joint);
                }
            }
        }
        self.diagnostics.remove(&chain);
    }

    fn release_resolved_ownership(&mut self, chain: ComponentId) {
        let ids = self
            .chains
            .get_mut(&chain)
            .map(|runtime| std::mem::take(&mut runtime.resolved_ids))
            .unwrap_or_default();
        for id in ids {
            if self.joint_claims.get(&id) == Some(&chain) {
                self.joint_claims.remove(&id);
            }
            if let Some(owners) = self.resolved_transform_chains.get_mut(&id) {
                owners.remove(&chain);
                if owners.is_empty() {
                    self.resolved_transform_chains.remove(&id);
                }
            }
        }
    }

    fn diagnose(&mut self, chain: ComponentId) {
        let message = match &self.chains[&chain].status {
            ChainStatus::WaitingForDependencies(message) | ChainStatus::Invalid(message) => message,
            ChainStatus::Bound(_) => return,
        };
        if self.diagnostics.get(&chain) == Some(message) {
            return;
        }
        eprintln!("[SecondaryMotion] chain {chain:?}: {message}");
        self.diagnostics.insert(chain, message.clone());
    }

    fn collision_frame(&mut self, world: &World) -> CollisionFrame {
        let revision = world.spring_collision_revision();
        if self.collider_pool_revision != Some(revision) {
            self.collider_pool.clear();
            let mut configs: Vec<_> = world.spring_collider_ids().collect();
            configs.sort_unstable();
            for config in configs {
                let Some(armature_owner) = nearest_armature_owner(world, config) else {
                    continue;
                };
                let settings = world
                    .get_component_by_id_as::<SpringColliderComponent>(config)
                    .unwrap();
                if !settings.radius.is_finite() || settings.radius <= 0.0 {
                    continue;
                }
                let mut seen = HashSet::new();
                for target in &settings.targets {
                    if let Ok(target) = resolve_in_armature(world, armature_owner, target)
                        && seen.insert(target)
                    {
                        self.collider_pool.push((
                            armature_owner,
                            BoundCollider {
                                config,
                                target,
                                radius: settings.radius,
                            },
                        ));
                    }
                }
            }
            self.collider_pool_revision = Some(revision);
        }
        let mut by_source = HashMap::new();
        let mut by_x = Vec::new();
        let mut max_radius = 0.0_f32;
        for (armature_owner, collider) in &self.collider_pool {
            let Some(center) = pos(world, collider.target) else {
                continue;
            };
            let radius = collider.radius * world_max_scale(world, collider.target);
            if !center.iter().all(|x| x.is_finite()) || !radius.is_finite() {
                continue;
            }
            let sphere = WorldSphere {
                armature_owner: *armature_owner,
                center,
                radius,
            };
            by_source.insert((collider.config, collider.target), sphere);
            by_x.push(sphere);
            max_radius = max_radius.max(radius);
        }
        // Stable sort preserves config/target order for coincident spheres.
        by_x.sort_by(|a, b| a.center[0].total_cmp(&b.center[0]));
        CollisionFrame {
            by_source,
            by_x,
            max_radius,
        }
    }

    /// Advances only retained bound chains and cached joints.
    pub fn tick(&mut self, world: &mut World, dt: f32) -> Vec<ComponentId> {
        self.tick_with_constraints(world, dt, &HashMap::new())
    }

    pub fn tick_with_constraints(
        &mut self,
        world: &mut World,
        dt: f32,
        rest_weights: &HashMap<ComponentId, f32>,
    ) -> Vec<ComponentId> {
        self.tick_with_settings(world, dt, rest_weights, true)
    }

    /// The global setting gates simulation without changing authored chain flags.
    /// Restore imported rotations once on pause; subsequent animation is free to
    /// update them. Retained disabled solver state resets from the live pose on resume.
    pub fn tick_with_settings(
        &mut self,
        world: &mut World,
        dt: f32,
        rest_weights: &HashMap<ComponentId, f32>,
        globally_enabled: bool,
    ) -> Vec<ComponentId> {
        self.tick_with_playback_settings(world, dt, rest_weights, globally_enabled, false)
    }

    pub fn tick_with_playback_settings(
        &mut self,
        world: &mut World,
        dt: f32,
        rest_weights: &HashMap<ComponentId, f32>,
        globally_enabled: bool,
        paused: bool,
    ) -> Vec<ComponentId> {
        self.tick_with_collisions(world, dt, rest_weights, globally_enabled, paused, None)
    }

    pub(crate) fn tick_with_collisions(
        &mut self,
        world: &mut World,
        dt: f32,
        rest_weights: &HashMap<ComponentId, f32>,
        globally_enabled: bool,
        paused: bool,
        surfaces: Option<&CollidableBvh>,
    ) -> Vec<ComponentId> {
        // Disabling still restores rest pose, even when the pause toggle is on.
        if globally_enabled && paused {
            self.paused = true;
            return Vec::new();
        }
        let resuming = std::mem::replace(&mut self.paused, false);
        if self.chains.is_empty() {
            return Vec::new();
        }
        let started = Instant::now();
        self.debug_frames = self.debug_frames.wrapping_add(1);
        let collision_frame = if globally_enabled {
            self.collision_frame(world)
        } else {
            CollisionFrame::default()
        };
        let mut max_correction_radians = 0.0f32;
        let mut dirty_roots = Vec::new();
        let mut dirty_set = HashSet::new();
        let mut stale = Vec::new();
        let mut collider_stale = Vec::new();
        for (&chain_id, runtime) in &mut self.chains {
            let ChainStatus::Bound(state) = &mut runtime.status else {
                continue;
            };
            if world.get_component_record(chain_id).is_none()
                || state
                    .joints
                    .iter()
                    .any(|joint| world.get_component_record(joint.id).is_none())
            {
                stale.push(chain_id);
                continue;
            }
            if state.colliders.iter().any(|collider| {
                world.get_component_record(collider.config).is_none()
                    || world.get_component_record(collider.target).is_none()
            }) {
                collider_stale.push(chain_id);
                continue;
            }
            let enabled = world
                .get_component_by_id_as::<SpringBoneComponent>(chain_id)
                .map(|chain| chain.enabled)
                .unwrap_or(false);
            if !enabled {
                state.enabled = false;
                continue;
            }
            if !globally_enabled {
                if state.enabled {
                    for joint in &state.joints {
                        if state.allow_stretch {
                            stretch::restore_translation(world, joint.id);
                        }
                        if let Some(transform) =
                            world.get_component_by_id_as_mut::<TransformComponent>(joint.id)
                        {
                            transform.transform.rotation = joint.rest_rotation;
                            transform.transform.recompute_model();
                        }
                    }
                    if let Some(root) = state.joints.first().map(|joint| joint.id)
                        && dirty_set.insert(root)
                    {
                        dirty_roots.push(root);
                    }
                }
                state.enabled = false;
                state.parked = false;
                state.accumulator = 0.0;
                continue;
            }
            let rest_weight = rest_weights
                .get(&chain_id)
                .copied()
                .unwrap_or(0.0)
                .clamp(0.0, 1.0);
            if resuming {
                // Rebase tails to the retained live pose after parent motion during
                // the pause. Discard stale velocity/time without restoring bind rotations.
                reset_state(world, state);
            }
            if !state.enabled || !dt.is_finite() || dt > 0.25 {
                reset_state(world, state);
                state.enabled = true;
                if rest_weight == 0.0 {
                    continue;
                }
            }
            if rest_weight < 1.0 {
                if state.parked {
                    reset_state(world, state);
                }
                state.accumulator = (state.accumulator + dt.max(0.0)).min(STEP * 4.0);
                let settings = world
                    .get_component_by_id_as::<SpringBoneComponent>(chain_id)
                    .unwrap();
                let contacts = chain_contacts(world, state, settings, &collision_frame, surfaces);
                while state.accumulator >= STEP {
                    if state.allow_stretch {
                        stretch::step(world, state);
                    } else {
                        simulate_step(world, state, &contacts);
                    }
                    state.accumulator -= STEP;
                }
            }
            state.parked = rest_weight >= 1.0;
            max_correction_radians =
                max_correction_radians.max(apply_rotations(world, state, rest_weight));
            if state.allow_stretch {
                stretch::apply(world, state, rest_weight);
            }
            if let Some(root) = state.joints.first().map(|joint| joint.id)
                && dirty_set.insert(root)
            {
                dirty_roots.push(root);
            }
        }
        for chain in stale {
            self.component_removed(world, chain);
        }
        for chain in collider_stale {
            self.bind_chain(world, chain);
        }
        self.profile.simulation += started.elapsed();
        if self.debug_frames % 120 == 0 && std::env::var_os("CAT_DEBUG_SECONDARY_MOTION").is_some()
        {
            let mut bound = 0;
            let mut waiting = 0;
            let mut invalid = 0;
            for runtime in self.chains.values() {
                match runtime.status {
                    ChainStatus::Bound(_) => bound += 1,
                    ChainStatus::WaitingForDependencies(_) => waiting += 1,
                    ChainStatus::Invalid(_) => invalid += 1,
                }
            }
            eprintln!(
                "[SecondaryMotion][debug] roots={} children={} bound={} waiting={} invalid={} max_rotation_correction_deg={:.2} binding_ms={:.3} invalidation_ms={:.3} simulation_ms={:.3} binding_attempts={} topology_discoveries={} selector_resolutions={}",
                self.roots.len(),
                self.child_owner.len(),
                bound,
                waiting,
                invalid,
                max_correction_radians.to_degrees(),
                self.profile.binding.as_secs_f64() * 1000.0,
                self.profile.invalidation.as_secs_f64() * 1000.0,
                self.profile.simulation.as_secs_f64() * 1000.0,
                self.profile.binding_attempts,
                self.profile.topology_discoveries,
                self.profile.selector_resolutions,
            );
        }
        dirty_roots
    }

    #[cfg(test)]
    pub(crate) fn discovery_counts(&self) -> (u64, u64, u64) {
        (
            self.profile.binding_attempts,
            self.profile.topology_discoveries,
            self.profile.selector_resolutions,
        )
    }

    #[cfg(test)]
    pub(crate) fn runtime_counts(&self) -> (usize, usize, usize, usize, usize) {
        let (mut bound, mut waiting, mut invalid) = (0, 0, 0);
        for runtime in self.chains.values() {
            match runtime.status {
                ChainStatus::Bound(_) => bound += 1,
                ChainStatus::WaitingForDependencies(_) => waiting += 1,
                ChainStatus::Invalid(_) => invalid += 1,
            }
        }
        (
            self.roots.len(),
            self.child_owner.len(),
            bound,
            waiting,
            invalid,
        )
    }
}

fn secondary_motion_parent_changed(
    _world: &mut World,
    emit: &mut dyn SignalEmitter,
    signal: &Signal,
) {
    if let Some(EventSignal::ParentChanged {
        child,
        old_parent,
        new_parent,
    }) = signal.event.as_ref()
    {
        for component in [Some(*child), *old_parent, *new_parent]
            .into_iter()
            .flatten()
        {
            emit.push_intent_now(
                component,
                IntentValue::SecondaryMotionTopologyChanged {
                    component_id: component,
                },
            );
        }
    }
}

fn secondary_motion_gltf_initialized(
    _world: &mut World,
    emit: &mut dyn SignalEmitter,
    signal: &Signal,
) {
    if let Some(EventSignal::GltfInitialized {
        gltf: armature_owner,
        ..
    }) = signal.event.as_ref()
    {
        emit.push_intent_now(
            *armature_owner,
            IntentValue::SecondaryMotionGltfInitialized {
                component_id: *armature_owner,
            },
        );
    }
}

fn reset_state(world: &World, state: &mut BoundChain) {
    for index in 0..state.joints.len() {
        let head = pos(world, state.joints[index].id).unwrap_or([0.0; 3]);
        let tail = if index + 1 < state.joints.len() {
            pos(world, state.joints[index + 1].id).unwrap_or(head)
        } else if index > 0 {
            let previous = pos(world, state.joints[index - 1].id).unwrap_or(head);
            vec3_add(
                head,
                vec3_scale(
                    vec3_normalize(vec3_sub(head, previous)),
                    state.lengths[index],
                ),
            )
        } else {
            head
        };
        state.current[index] = tail;
        state.previous[index] = tail;
    }
    state.accumulator = 0.0;
}

fn chain_contacts<'a>(
    world: &World,
    state: &BoundChain,
    settings: &SpringBoneComponent,
    frame: &CollisionFrame,
    surfaces: Option<&'a CollidableBvh>,
) -> ChainContacts<'a> {
    let hit_radius = state.hit_radius
        * owner_anchor(world, state.armature_owner)
            .map(|anchor| world_max_scale(world, anchor))
            .unwrap_or(1.0);
    let center = pos(world, state.joints[0].id).unwrap_or(state.current[0]);
    let reach = state.lengths.iter().sum::<f32>() + hit_radius;
    let mut spheres: Vec<_> = state
        .colliders
        .iter()
        .filter_map(|collider| {
            frame
                .by_source
                .get(&(collider.config, collider.target))
                .copied()
        })
        .collect();
    if settings.avatar_collision {
        let extent = reach + frame.max_radius;
        let lo = frame
            .by_x
            .partition_point(|s| s.center[0] < center[0] - extent);
        let hi = frame
            .by_x
            .partition_point(|s| s.center[0] <= center[0] + extent);
        spheres.extend(
            frame.by_x[lo..hi]
                .iter()
                .filter(|sphere| {
                    sphere.armature_owner != state.armature_owner
                        && vec3_len(vec3_sub(sphere.center, center)) <= reach + sphere.radius
                })
                .copied(),
        );
    }
    let surfaces = surfaces.filter(|_| settings.world_collision);
    let mut surface_ids = Vec::new();
    if let Some(surfaces) = surfaces {
        // The entire reachable chain, including old tails for downward sweeps.
        let mut min = center.map(|v| v - reach);
        let mut max = center.map(|v| v + reach);
        for point in state.current.iter().chain(&state.previous) {
            for axis in 0..3 {
                min[axis] = min[axis].min(point[axis] - hit_radius);
                max[axis] = max[axis].max(point[axis] + hit_radius);
            }
        }
        surfaces.query(min, max, &mut Vec::new(), &mut surface_ids);
        surface_ids.retain(|&id| {
            let surface = surfaces.surface(id);
            nearest_armature_owner(world, surface.collidable) != Some(state.armature_owner)
                && !owns_frame(world, state.armature_owner, surface.frame_id)
        });
    }
    ChainContacts {
        spheres,
        surfaces,
        surface_ids,
        hit_radius,
    }
}

fn simulate_step(world: &World, state: &mut BoundChain, contacts: &ChainContacts<'_>) {
    let rest_frame = state
        .rest_targets
        .as_ref()
        .map(|targets| targets.rotation(world));
    for index in 0..state.joints.len() {
        let primary_head = pos(world, state.joints[index].id).unwrap_or(state.current[index]);
        let head = if index == 0 {
            primary_head
        } else {
            state.current[index - 1]
        };
        let inertia = vec3_scale(
            vec3_sub(state.current[index], state.previous[index]),
            1.0 - state.joints[index].drag,
        );
        let rest_tail = if let (Some(targets), Some(frame)) = (&state.rest_targets, rest_frame) {
            vec3_add(
                head,
                vec3_scale(
                    quat_rotate_vec3(frame, targets.directions[index]),
                    state.lengths[index],
                ),
            )
        } else if index + 1 < state.joints.len() {
            let primary_tail =
                pos(world, state.joints[index + 1].id).unwrap_or(state.current[index]);
            vec3_add(
                head,
                vec3_scale(
                    vec3_normalize(vec3_sub(primary_tail, primary_head)),
                    state.lengths[index],
                ),
            )
        } else if index > 0 {
            let previous = pos(world, state.joints[index - 1].id).unwrap_or(head);
            vec3_add(
                head,
                vec3_scale(
                    vec3_normalize(vec3_sub(head, previous)),
                    state.lengths[index],
                ),
            )
        } else {
            state.current[index]
        };
        let stiffness = vec3_scale(
            vec3_sub(rest_tail, state.current[index]),
            state.joints[index].stiffness * STEP,
        );
        let next = vec3_add(
            vec3_add(state.current[index], inertia),
            vec3_add(
                stiffness,
                vec3_scale(state.joints[index].gravity, STEP * STEP),
            ),
        );
        state.previous[index] = state.current[index];
        let direction = vec3_normalize(vec3_sub(next, head));
        state.current[index] = vec3_add(head, vec3_scale(direction, state.lengths[index]));
    }
    if contacts.spheres.is_empty() && contacts.surface_ids.is_empty() {
        return;
    }
    for _ in 0..4 {
        for index in 0..state.joints.len() {
            let head = if index == 0 {
                pos(world, state.joints[index].id).unwrap_or(state.current[index])
            } else {
                state.current[index - 1]
            };
            // An earlier segment may have moved during this contact pass.
            // Re-anchor even when this endpoint has no contact of its own.
            let offset = vec3_sub(state.current[index], head);
            let direction = if vec3_len(offset) > f32::EPSILON {
                vec3_normalize(offset)
            } else {
                [1.0, 0.0, 0.0]
            };
            let mut endpoint = vec3_add(head, vec3_scale(direction, state.lengths[index]));
            for sphere in &contacts.spheres {
                let radius = sphere.radius + contacts.hit_radius;
                if vec3_len(vec3_sub(endpoint, sphere.center)) < radius {
                    endpoint = exclude_sphere_at_length(
                        head,
                        endpoint,
                        state.lengths[index],
                        sphere.center,
                        radius,
                    );
                }
            }
            if let Some(surfaces) = contacts.surfaces {
                for &surface_id in &contacts.surface_ids {
                    let surface = surfaces.surface(surface_id);
                    // A zero-segment capsule is a sphere; reuse the floor sweep
                    // so a falling tail cannot jump through a thin platform.
                    let shape = CollisionShape::CapsuleY {
                        radius: contacts.hit_radius,
                        half_segment: 0.0,
                    };
                    let swept = surface.floor_sweep(
                        state.joints[index].id,
                        endpoint,
                        shape,
                        state.previous[index],
                    );
                    let delta = if let Some(hit) = swept {
                        Some([0.0, hit.point[1] + contacts.hit_radius - endpoint[1], 0.0])
                    } else {
                        surface.translation(
                            endpoint,
                            CollisionShape::Sphere {
                                radius: contacts.hit_radius,
                            },
                        )
                    };
                    if let Some(delta) = delta
                        && vec3_len(delta) > 1e-7
                    {
                        endpoint = exclude_plane_at_length(
                            head,
                            endpoint,
                            state.lengths[index],
                            vec3_add(endpoint, delta),
                            vec3_normalize(delta),
                        );
                    }
                }
            }
            state.current[index] = endpoint;
        }
    }
}

/// Project onto a contact half-space while keeping the bone length exactly.
/// A simple push followed by normalization sinks a vertical tail into a floor.
fn exclude_plane_at_length(
    head: [f32; 3],
    endpoint: [f32; 3],
    length: f32,
    point: [f32; 3],
    normal: [f32; 3],
) -> [f32; 3] {
    if length <= f32::EPSILON {
        return head;
    }
    let dot = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).map(|(a, b)| a * b).sum::<f32>();
    let desired = vec3_normalize(vec3_sub(endpoint, head));
    let boundary = (dot(vec3_sub(point, head), normal) / length).clamp(-1.0, 1.0);
    let cosine = dot(desired, normal);
    if cosine >= boundary {
        return vec3_add(head, vec3_scale(desired, length));
    }
    let mut tangent = vec3_sub(desired, vec3_scale(normal, cosine));
    if vec3_len(tangent) <= f32::EPSILON {
        let axis = if normal[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 0.0, 1.0]
        };
        tangent = vec3_sub(axis, vec3_scale(normal, dot(axis, normal)));
    }
    let direction = vec3_add(
        vec3_scale(normal, boundary),
        vec3_scale(
            vec3_normalize(tangent),
            (1.0 - boundary * boundary).max(0.0).sqrt(),
        ),
    );
    vec3_add(head, vec3_scale(direction, length))
}

fn exclude_sphere_at_length(
    head: [f32; 3],
    endpoint: [f32; 3],
    length: f32,
    center: [f32; 3],
    radius: f32,
) -> [f32; 3] {
    if length <= f32::EPSILON {
        return endpoint;
    }
    let to_center = vec3_sub(center, head);
    let center_distance = vec3_len(to_center);
    let desired = {
        let delta = vec3_sub(endpoint, head);
        if vec3_len(delta) > f32::EPSILON {
            vec3_normalize(delta)
        } else {
            [1.0, 0.0, 0.0]
        }
    };
    if center_distance <= f32::EPSILON {
        return vec3_add(head, vec3_scale(desired, length));
    }
    let axis = vec3_scale(to_center, 1.0 / center_distance);
    // Points on the segment-length sphere are outside the collider when their
    // direction has dot(axis) <= this boundary cosine.
    let boundary_cos = ((center_distance * center_distance + length * length - radius * radius)
        / (2.0 * center_distance * length))
        .clamp(-1.0, 1.0);
    let desired_cos = desired[0] * axis[0] + desired[1] * axis[1] + desired[2] * axis[2];
    if desired_cos <= boundary_cos {
        return vec3_add(head, vec3_scale(desired, length));
    }
    let mut perpendicular = vec3_sub(desired, vec3_scale(axis, desired_cos));
    if vec3_len(perpendicular) <= f32::EPSILON {
        let basis = if axis[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let projection = axis[0] * basis[0] + axis[1] * basis[1] + axis[2] * basis[2];
        perpendicular = vec3_sub(basis, vec3_scale(axis, projection));
    }
    perpendicular = vec3_normalize(perpendicular);
    let tangent = (1.0 - boundary_cos * boundary_cos).max(0.0).sqrt();
    let direction = vec3_add(
        vec3_scale(axis, boundary_cos),
        vec3_scale(perpendicular, tangent),
    );
    vec3_add(head, vec3_scale(direction, length))
}

fn apply_rotations(world: &mut World, state: &BoundChain, rest_weight: f32) -> f32 {
    let mut max_correction = 0.0f32;
    let mut previous_joint_world_q = None;
    for index in 0..state.joints.len() {
        let joint = &state.joints[index];
        let parent_q = if index > 0 && joint.parent_id == state.joints[index - 1].id {
            previous_joint_world_q.unwrap_or([0.0, 0.0, 0.0, 1.0])
        } else {
            world
                .get_component_by_id_as::<TransformComponent>(joint.parent_id)
                .map(|transform| mat_to_quat(transform.transform.matrix_world))
                .unwrap_or([0.0, 0.0, 0.0, 1.0])
        };
        let head = if index == 0 {
            let Some(head) = pos(world, joint.id) else {
                continue;
            };
            head
        } else {
            state.current[index - 1]
        };
        let desired_local = quat_rotate_vec3(
            quat_conjugate(parent_q),
            vec3_normalize(vec3_sub(state.current[index], head)),
        );
        let rest_direction = if vec3_len(joint.rest_direction) > 0.0 {
            joint.rest_direction
        } else {
            desired_local
        };
        let correction = shortest_arc_quat(rest_direction, desired_local);
        max_correction = max_correction.max(2.0 * correction[3].abs().clamp(0.0, 1.0).acos());
        let simulated = quat_mul(correction, joint.rest_rotation);
        let rest = joint.rest_rotation;
        let sign = if simulated.iter().zip(rest).map(|(a, b)| a * b).sum::<f32>() < 0.0 {
            -1.0
        } else {
            1.0
        };
        let rotation = quat_normalize([
            simulated[0] * (1.0 - rest_weight) + rest[0] * rest_weight * sign,
            simulated[1] * (1.0 - rest_weight) + rest[1] * rest_weight * sign,
            simulated[2] * (1.0 - rest_weight) + rest[2] * rest_weight * sign,
            simulated[3] * (1.0 - rest_weight) + rest[3] * rest_weight * sign,
        ]);
        previous_joint_world_q = Some(quat_mul(parent_q, rotation));
        if let Some(transform) = world.get_component_by_id_as_mut::<TransformComponent>(joint.id) {
            transform.transform.rotation = rotation;
            transform.transform.recompute_model();
        }
    }
    max_correction
}

#[cfg(test)]
#[path = "secondary_motion_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "secondary_motion_collision_tests.rs"]
mod collision_tests;
