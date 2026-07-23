use crate::engine::ecs::component::{
    BoneRestPoseComponent, ComponentRef, GLTFComponent, QueryRootMode, SecondaryMotionComponent,
    SpringBoneComponent, SpringColliderComponent, SpringJointComponent, TransformComponent,
    resolve_component_ref,
};
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, RxWorld, Signal, SignalEmitter, SignalKind, World,
};
use crate::utils::math::{
    mat_to_quat, quat_conjugate, quat_mul, quat_rotate_vec3, shortest_arc_quat, vec3_add, vec3_len,
    vec3_normalize, vec3_scale, vec3_sub,
};
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
    previous: Vec<[f32; 3]>,
    current: Vec<[f32; 3]>,
    lengths: Vec<f32>,
    accumulator: f32,
    enabled: bool,
    gltf: ComponentId,
    hit_radius: f32,
    colliders: Vec<BoundCollider>,
}

#[derive(Debug, Clone)]
struct BoundCollider {
    config: ComponentId,
    target: ComponentId,
    radius: f32,
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
    gltf: Option<ComponentId>,
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
    registered: HashSet<ComponentId>,
    roots: HashMap<ComponentId, RootRuntime>,
    chains: HashMap<ComponentId, ChainRuntime>,
    child_owner: HashMap<ComponentId, ComponentId>,
    joint_owner: HashMap<ComponentId, ComponentId>,
    gltf_roots: HashMap<ComponentId, HashSet<ComponentId>>,
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
    pub gltf: ComponentId,
    pub chain: ComponentId,
    pub enabled: bool,
    pub hit_radius: f32,
    pub scaled_hit_radius: f32,
    pub segments: Vec<SecondaryMotionSegmentSnapshot>,
    pub colliders: Vec<SecondaryMotionColliderSnapshot>,
}

impl SecondaryMotionSystem {
    /// Read-only projection of successfully bound solver state in current world space.
    pub fn bound_snapshot(&self, world: &World) -> Vec<SecondaryMotionChainSnapshot> {
        let mut snapshot = Vec::new();
        for (&chain, runtime) in &self.chains {
            let ChainStatus::Bound(state) = &runtime.status else {
                continue;
            };
            let hit_scale = world
                .parent_of(state.gltf)
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
                gltf: state.gltf,
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
            let owning_gltf = nearest_gltf(world, component);
            let chains: Vec<_> = self
                .chains
                .iter()
                .filter_map(|(&chain, runtime)| {
                    let root = runtime.root?;
                    (self.roots.get(&root)?.gltf == owning_gltf
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
            let owning_gltf = nearest_gltf(world, component);
            for (&chain, runtime) in &self.chains {
                let same_gltf = runtime
                    .root
                    .and_then(|root| self.roots.get(&root))
                    .and_then(|root| root.gltf)
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
        if let Some(roots) = self.gltf_roots.get(&component) {
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
            let Some(gltf) = runtime
                .root
                .and_then(|root| self.roots.get(&root))
                .and_then(|root| root.gltf)
            else {
                continue;
            };
            if world
                .get_component_by_id_as::<GLTFComponent>(gltf)
                .is_some_and(|gltf| gltf.armature_joint_transforms.contains(&component))
            {
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

    pub fn gltf_initialized(&mut self, world: &World, gltf: ComponentId) {
        let roots: Vec<_> = self
            .gltf_roots
            .get(&gltf)
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
        if let Some(roots) = self.gltf_roots.remove(&component) {
            for root in roots {
                if let Some(runtime) = self.roots.get_mut(&root) {
                    runtime.gltf = None;
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
        if let Some(roots) = self.gltf_roots.get(&target) {
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
        if let Some(gltf) = runtime.gltf
            && let Some(roots) = self.gltf_roots.get_mut(&gltf)
        {
            roots.remove(&root);
            if roots.is_empty() {
                self.gltf_roots.remove(&gltf);
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

    fn reconcile_root(&mut self, world: &World, root: ComponentId) {
        self.profile.topology_discoveries += 1;
        let new_gltf = nearest_gltf(world, root);
        let old_gltf = self.roots.get(&root).and_then(|runtime| runtime.gltf);
        if old_gltf != new_gltf {
            if let Some(old) = old_gltf
                && let Some(roots) = self.gltf_roots.get_mut(&old)
            {
                roots.remove(&root);
                if roots.is_empty() {
                    self.gltf_roots.remove(&old);
                }
            }
            if let Some(new) = new_gltf {
                self.gltf_roots.entry(new).or_default().insert(root);
            }
            self.roots.get_mut(&root).unwrap().gltf = new_gltf;
        }
        let chains: Vec<_> = self.roots[&root].children.iter().copied().collect();
        for chain in chains {
            self.bind_chain(world, chain);
        }
    }

    fn reconcile_chain(&mut self, world: &World, chain: ComponentId) {
        if !self.chains.contains_key(&chain) {
            return;
        }
        let old_root = self.child_owner.remove(&chain);
        if let Some(root) = old_root
            && let Some(runtime) = self.roots.get_mut(&root)
        {
            runtime.children.remove(&chain);
        }

        self.profile.topology_discoveries += 2;
        let parent = world.parent_of(chain);
        let new_root = parent.filter(|id| {
            world
                .get_component_by_id_as::<SecondaryMotionComponent>(*id)
                .is_some()
        });
        if old_root != new_root {
            self.release_resolved_ownership(chain);
            if let Some(old_root) = old_root {
                self.retry_invalid_children(world, old_root, chain);
            }
        }
        let Some(root) = new_root else {
            self.release_resolved_ownership(chain);
            let runtime = self.chains.get_mut(&chain).unwrap();
            runtime.root = None;
            runtime.status =
                ChainStatus::Invalid("SpringBone must be a direct child of SecondaryMotion".into());
            self.diagnose(chain);
            return;
        };
        if !self.roots.contains_key(&root) {
            self.registered.insert(root);
            self.roots.entry(root).or_default();
            self.reconcile_root(world, root);
        }
        self.child_owner.insert(chain, root);
        self.roots.get_mut(&root).unwrap().children.insert(chain);
        self.chains.get_mut(&chain).unwrap().root = Some(root);

        let old_joints = std::mem::take(&mut self.chains.get_mut(&chain).unwrap().joint_config_ids);
        for joint in old_joints {
            if self.joint_owner.get(&joint) == Some(&chain) {
                self.joint_owner.remove(&joint);
            }
        }
        let joints: Vec<_> = world
            .children_of(chain)
            .iter()
            .copied()
            .filter(|id| {
                world
                    .get_component_by_id_as::<SpringJointComponent>(*id)
                    .is_some()
            })
            .collect();
        for joint in &joints {
            self.registered.insert(*joint);
            self.joint_owner.insert(*joint, chain);
        }
        self.chains.get_mut(&chain).unwrap().joint_config_ids = joints;
        self.bind_chain(world, chain);
    }

    fn retry_invalid_children(&mut self, world: &World, root: ComponentId, except: ComponentId) {
        let retry: Vec<_> = self
            .roots
            .get(&root)
            .into_iter()
            .flat_map(|runtime| runtime.children.iter())
            .copied()
            .filter(|candidate| {
                *candidate != except
                    && matches!(self.chains[candidate].status, ChainStatus::Invalid(_))
            })
            .collect();
        for candidate in retry {
            self.bind_chain(world, candidate);
        }
    }

    fn bind_chain(&mut self, world: &World, chain: ComponentId) {
        let started = Instant::now();
        self.profile.binding_attempts += 1;
        self.release_resolved_ownership(chain);
        let Some(root) = self.chains.get(&chain).and_then(|runtime| runtime.root) else {
            return;
        };
        let Some(gltf) = self.roots.get(&root).and_then(|runtime| runtime.gltf) else {
            self.chains.get_mut(&chain).unwrap().status =
                ChainStatus::WaitingForDependencies("SecondaryMotion has no owning GLTF".into());
            self.diagnose(chain);
            self.profile.binding += started.elapsed();
            return;
        };
        let joint_config_ids = self.chains[&chain].joint_config_ids.clone();
        self.profile.selector_resolutions += joint_config_ids.len() as u64
            + u64::from(
                world
                    .get_component_by_id_as::<SpringBoneComponent>(chain)
                    .is_some_and(|chain| chain.center.is_some()),
            )
            + u64::from(
                world
                    .get_component_by_id_as::<SpringBoneComponent>(chain)
                    .is_some_and(|chain| chain.root.is_some() && joint_config_ids.is_empty()),
            );
        match build_chain(world, gltf, chain, &joint_config_ids) {
            Ok((bound, warnings)) => {
                for warning in warnings {
                    if self.collider_diagnostics.insert(warning.clone()) {
                        eprintln!("[SecondaryMotion] {warning}");
                    }
                }
                let resolved_ids: Vec<_> = bound.joints.iter().map(|joint| joint.id).collect();
                let overlap = bound.joints.iter().find_map(|joint| {
                    self.joint_claims
                        .get(&joint.id)
                        .copied()
                        .filter(|owner| *owner != chain)
                });
                self.chains.get_mut(&chain).unwrap().resolved_ids = resolved_ids.clone();
                for id in resolved_ids {
                    self.resolved_transform_chains
                        .entry(id)
                        .or_default()
                        .insert(chain);
                }
                if let Some(owner) = overlap {
                    self.chains.get_mut(&chain).unwrap().status =
                        ChainStatus::Invalid(format!("resolved joint overlaps chain {owner:?}"));
                    self.diagnose(chain);
                } else {
                    for joint in &bound.joints {
                        self.joint_claims.insert(joint.id, chain);
                    }
                    self.chains.get_mut(&chain).unwrap().status = ChainStatus::Bound(bound);
                    self.diagnostics.remove(&chain);
                }
            }
            Err(failure) => {
                self.chains.get_mut(&chain).unwrap().status = if failure.waiting {
                    ChainStatus::WaitingForDependencies(failure.message)
                } else {
                    ChainStatus::Invalid(failure.message)
                };
                self.diagnose(chain);
            }
        }
        self.profile.binding += started.elapsed();
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

    /// Advances only retained bound chains and cached joints.
    pub fn tick(&mut self, world: &mut World, dt: f32) -> Vec<ComponentId> {
        if self.chains.is_empty() {
            return Vec::new();
        }
        let started = Instant::now();
        self.debug_frames = self.debug_frames.wrapping_add(1);
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
            if !state.enabled || !dt.is_finite() || dt > 0.25 {
                reset_state(world, state);
                state.enabled = true;
                continue;
            }
            state.accumulator = (state.accumulator + dt.max(0.0)).min(STEP * 4.0);
            while state.accumulator >= STEP {
                simulate_step(world, state);
                state.accumulator -= STEP;
            }
            max_correction_radians = max_correction_radians.max(apply_rotations(world, state));
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
                    component_ids: vec![component],
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
    if let Some(EventSignal::GltfInitialized { gltf, .. }) = signal.event.as_ref() {
        emit.push_intent_now(
            *gltf,
            IntentValue::SecondaryMotionGltfInitialized {
                component_ids: vec![*gltf],
            },
        );
    }
}

fn nearest_gltf(world: &World, mut id: ComponentId) -> Option<ComponentId> {
    for _ in 0..64 {
        id = world.parent_of(id)?;
        if world.get_component_by_id_as::<GLTFComponent>(id).is_some() {
            return Some(id);
        }
    }
    None
}

fn descendant(world: &World, mut node: ComponentId, ancestor: ComponentId) -> bool {
    for _ in 0..64 {
        if node == ancestor {
            return true;
        }
        let Some(parent) = world.parent_of(node) else {
            return false;
        };
        node = parent;
    }
    false
}

fn pos(world: &World, id: ComponentId) -> Option<[f32; 3]> {
    let matrix = world
        .get_component_by_id_as::<TransformComponent>(id)?
        .transform
        .matrix_world;
    Some([matrix[3][0], matrix[3][1], matrix[3][2]])
}

fn rest(world: &World, id: ComponentId) -> Option<BoneRestPoseComponent> {
    world.children_of(id).iter().find_map(|child| {
        world
            .get_component_by_id_as::<BoneRestPoseComponent>(*child)
            .copied()
    })
}

fn ref_surface(reference: &ComponentRef) -> String {
    match reference {
        ComponentRef::Guid(guid) => format!("@uuid:{guid}"),
        ComponentRef::Query(query) => query.clone(),
    }
}

#[derive(Debug)]
struct BindFailure {
    waiting: bool,
    message: String,
}

impl BindFailure {
    fn waiting(message: impl Into<String>) -> Self {
        Self {
            waiting: true,
            message: message.into(),
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self {
            waiting: false,
            message: message.into(),
        }
    }
}

fn resolve_in_gltf(
    world: &World,
    gltf_id: ComponentId,
    reference: &ComponentRef,
) -> Result<ComponentId, BindFailure> {
    let gltf = world
        .get_component_by_id_as::<GLTFComponent>(gltf_id)
        .ok_or_else(|| BindFailure::waiting("owning GLTF disappeared"))?;
    let instance_nodes: HashSet<_> = gltf.spawned_node_transforms.iter().copied().collect();
    if instance_nodes.is_empty() {
        return Err(BindFailure::waiting(
            "owning GLTF has not spawned its node transforms yet",
        ));
    }
    let anchor = world
        .parent_of(gltf_id)
        .ok_or_else(|| BindFailure::waiting("owning GLTF has no transform anchor"))?;
    let id = match reference {
        ComponentRef::Guid(guid) => world.component_id_by_guid(*guid),
        ComponentRef::Query(query) if !query.starts_with('/') && !query.starts_with("../") => {
            let matches: Vec<_> = instance_nodes
                .iter()
                .copied()
                .filter(|id| world.component_matches_selector(*id, query))
                .collect();
            if matches.len() != 1 {
                return Err(BindFailure::invalid(format!(
                    "query '{}' matched {} nodes in the owning GLTF instance (expected exactly one)",
                    query,
                    matches.len()
                )));
            }
            matches.first().copied()
        }
        ComponentRef::Query(_) => resolve_component_ref(
            world,
            reference,
            Some(anchor),
            QueryRootMode::SelfSubtree,
        ),
    }
    .ok_or_else(|| {
        BindFailure::waiting(format!(
            "reference '{}' did not resolve",
            ref_surface(reference)
        ))
    })?;
    if !instance_nodes.contains(&id) {
        return Err(BindFailure::invalid(format!(
            "reference '{}' resolved outside the owning GLTF instance",
            ref_surface(reference)
        )));
    }
    Ok(id)
}

fn resolve_collider_config_in_gltf(
    world: &World,
    gltf_id: ComponentId,
    reference: &ComponentRef,
) -> Result<ComponentId, String> {
    let id = match reference {
        ComponentRef::Guid(guid) => world.component_id_by_guid(*guid).ok_or_else(|| {
            format!(
                "collider configuration '{}' did not resolve",
                ref_surface(reference)
            )
        })?,
        ComponentRef::Query(query) => {
            let matches = world.find_all_components(gltf_id, query);
            if matches.len() != 1 {
                return Err(format!(
                    "collider query '{}' matched {} configurations in the owning GLTF instance (expected exactly one)",
                    query,
                    matches.len()
                ));
            }
            matches[0]
        }
    };
    if world
        .get_component_by_id_as::<SpringColliderComponent>(id)
        .is_none()
        || nearest_gltf(world, id) != Some(gltf_id)
    {
        return Err(format!(
            "collider configuration '{}' resolved outside the owning GLTF instance or is not a SpringCollider",
            ref_surface(reference)
        ));
    }
    Ok(id)
}

fn world_max_scale(world: &World, id: ComponentId) -> f32 {
    world
        .get_component_by_id_as::<TransformComponent>(id)
        .map(|transform| {
            let matrix = transform.transform.matrix_world;
            [
                vec3_len([matrix[0][0], matrix[0][1], matrix[0][2]]),
                vec3_len([matrix[1][0], matrix[1][1], matrix[1][2]]),
                vec3_len([matrix[2][0], matrix[2][1], matrix[2][2]]),
            ]
            .into_iter()
            .fold(0.0, f32::max)
        })
        .filter(|scale| scale.is_finite() && *scale > f32::EPSILON)
        .unwrap_or(1.0)
}

fn build_chain(
    world: &World,
    gltf_id: ComponentId,
    chain_id: ComponentId,
    joint_config_ids: &[ComponentId],
) -> Result<(BoundChain, Vec<String>), BindFailure> {
    if let Some(anchor) = world
        .parent_of(gltf_id)
        .and_then(|id| world.get_component_by_id_as::<TransformComponent>(id))
    {
        let matrix = anchor.transform.matrix_world;
        let det = matrix[0][0] * (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1])
            - matrix[1][0] * (matrix[0][1] * matrix[2][2] - matrix[0][2] * matrix[2][1])
            + matrix[2][0] * (matrix[0][1] * matrix[1][2] - matrix[0][2] * matrix[1][1]);
        if det <= 0.0 {
            return Err(BindFailure::invalid(
                "negative GLTF instance scale is unsupported",
            ));
        }
    }
    let chain = world
        .get_component_by_id_as::<SpringBoneComponent>(chain_id)
        .ok_or_else(|| BindFailure::waiting("missing SpringBone"))?;
    let mut warnings = Vec::new();
    let mut colliders = Vec::new();
    let mut seen_configs = HashSet::new();
    let mut seen_colliders = HashSet::new();
    for collider_ref in &chain.colliders {
        let config_id = match resolve_collider_config_in_gltf(world, gltf_id, collider_ref) {
            Ok(id) => id,
            Err(message) => {
                warnings.push(format!("chain {chain_id:?}: {message}; skipping"));
                continue;
            }
        };
        if !seen_configs.insert(config_id) {
            continue;
        }
        let config = world
            .get_component_by_id_as::<SpringColliderComponent>(config_id)
            .expect("validated collider configuration");
        for target_ref in &config.targets {
            match resolve_in_gltf(world, gltf_id, target_ref) {
                Ok(target) => {
                    if seen_colliders.insert((target, config.radius.to_bits())) {
                        colliders.push(BoundCollider {
                            config: config_id,
                            target,
                            radius: config.radius,
                        });
                    }
                }
                Err(failure) => warnings.push(format!(
                    "collider {config_id:?} target '{}': {}; skipping",
                    ref_surface(target_ref),
                    failure.message
                )),
            }
        }
    }
    if let Some(center) = &chain.center {
        resolve_in_gltf(world, gltf_id, center).map_err(|failure| BindFailure {
            waiting: failure.waiting,
            message: format!("center {}", failure.message),
        })?;
    }
    let auto_ids = if joint_config_ids.is_empty() {
        let root_ref = chain
            .root
            .as_ref()
            .ok_or_else(|| BindFailure::invalid("has no SpringJoint children"))?;
        let root_id = resolve_in_gltf(world, gltf_id, root_ref)?;
        let skin_joints: HashSet<_> = world
            .get_component_by_id_as::<GLTFComponent>(gltf_id)
            .ok_or_else(|| BindFailure::waiting("owning GLTF disappeared"))?
            .armature_joint_transforms
            .iter()
            .copied()
            .collect();
        if !skin_joints.contains(&root_id) {
            return Err(BindFailure::invalid(format!(
                "automatic root '{}' is not a joint in the owning GLTF skin",
                ref_surface(root_ref)
            )));
        }
        let mut ids = vec![root_id];
        let mut cursor = root_id;
        loop {
            // Imported collider and visualization helpers may be parented below a
            // bone. Only direct children belonging to the main skin define chain
            // topology.
            let children: Vec<_> = world
                .children_of(cursor)
                .iter()
                .copied()
                .filter(|id| skin_joints.contains(id))
                .collect();
            match children.as_slice() {
                [] => break,
                [child] => {
                    ids.push(*child);
                    cursor = *child;
                }
                _ => {
                    let labels = children
                        .iter()
                        .map(|id| world.component_label(*id).unwrap_or("<unnamed>"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(BindFailure::invalid(format!(
                        "automatic chain from '{}' branches at '{}' into skin joints [{}]",
                        ref_surface(root_ref),
                        world.component_label(cursor).unwrap_or("<unnamed>"),
                        labels
                    )));
                }
            }
        }
        ids
    } else {
        Vec::new()
    };
    let mut joints = Vec::new();
    let mut local_ids = HashSet::new();
    let mut resolved = Vec::new();
    if auto_ids.is_empty() {
        for joint_config_id in joint_config_ids {
            let config = world
                .get_component_by_id_as::<SpringJointComponent>(*joint_config_id)
                .ok_or_else(|| BindFailure::waiting("SpringJoint configuration disappeared"))?;
            resolved.push((
                resolve_in_gltf(world, gltf_id, &config.node)?,
                ref_surface(&config.node),
                config.stiffness,
                config.drag_force,
                config.gravity_power,
                config.gravity_dir,
            ));
        }
    } else {
        for id in auto_ids {
            resolved.push((
                id,
                world.component_label(id).unwrap_or("<unnamed>").to_string(),
                chain.stiffness,
                chain.drag_force,
                chain.gravity_power,
                chain.gravity_dir,
            ));
        }
    }
    for (id, surface, stiffness, drag_force, gravity_power, gravity_dir) in resolved {
        if !local_ids.insert(id) {
            return Err(BindFailure::invalid(format!(
                "joint '{}' occurs more than once in the chain",
                surface
            )));
        }
        let rest_pose = rest(world, id)
            .ok_or_else(|| BindFailure::waiting(format!("joint '{}' has no rest pose", surface)))?;
        let parent_id = world.parent_of(id).ok_or_else(|| {
            BindFailure::waiting(format!("joint '{}' has no transform parent", surface))
        })?;
        joints.push(JointConfig {
            id,
            parent_id,
            rest_rotation: rest_pose.rotation,
            rest_direction: [0.0; 3],
            stiffness,
            drag: drag_force,
            gravity: vec3_scale(vec3_normalize(gravity_dir), gravity_power),
        });
    }
    for pair in joints.windows(2) {
        if !descendant(world, pair[1].id, pair[0].id) {
            return Err(BindFailure::invalid(
                "joint list is reordered or non-ancestral",
            ));
        }
    }
    let virtual_ratio = chain.virtual_end_length_ratio;
    if joints.len() == 1 && virtual_ratio.is_none() {
        return Err(BindFailure::invalid(
            "a one-joint chain requires a virtual endpoint",
        ));
    }
    for index in 0..joints.len() {
        joints[index].rest_direction = if index + 1 < joints.len() {
            let next_rest = rest(world, joints[index + 1].id)
                .ok_or_else(|| BindFailure::waiting("next joint rest pose disappeared"))?;
            quat_rotate_vec3(
                joints[index].rest_rotation,
                vec3_normalize(next_rest.translation),
            )
        } else if index > 0 {
            vec3_normalize(
                rest(world, joints[index].id)
                    .ok_or_else(|| BindFailure::waiting("joint rest pose disappeared"))?
                    .translation,
            )
        } else {
            vec3_normalize(
                rest(world, joints[index].id)
                    .ok_or_else(|| BindFailure::waiting("joint rest pose disappeared"))?
                    .translation,
            )
        };
    }
    // Seed world-space tails from immutable rest data. Lifecycle registration can
    // run before the first transform propagation, so matrix_world is not a valid
    // source of segment length at this point.
    let instance_scale = world
        .parent_of(gltf_id)
        .and_then(|id| world.get_component_by_id_as::<TransformComponent>(id))
        .map(|anchor| {
            let matrix = anchor.transform.matrix_world;
            [
                vec3_len([matrix[0][0], matrix[0][1], matrix[0][2]]),
                vec3_len([matrix[1][0], matrix[1][1], matrix[1][2]]),
                vec3_len([matrix[2][0], matrix[2][1], matrix[2][2]]),
            ]
            .into_iter()
            .fold(0.0, f32::max)
        })
        .filter(|scale| *scale > f32::EPSILON)
        .unwrap_or(1.0);
    let mut current = Vec::new();
    let mut lengths = Vec::new();
    for index in 0..joints.len() {
        let head = if index == 0 {
            pos(world, joints[index].id)
                .ok_or_else(|| BindFailure::waiting("joint transform missing"))?
        } else {
            current[index - 1]
        };
        let length = if index + 1 < joints.len() {
            vec3_len(
                rest(world, joints[index + 1].id)
                    .ok_or_else(|| BindFailure::waiting("next joint rest pose disappeared"))?
                    .translation,
            ) * instance_scale
        } else if index > 0 {
            lengths[index - 1] * virtual_ratio.unwrap_or(1.0)
        } else {
            vec3_len(
                rest(world, joints[index].id)
                    .ok_or_else(|| BindFailure::waiting("joint rest pose disappeared"))?
                    .translation,
            ) * instance_scale
                * virtual_ratio.unwrap_or(1.0)
        };
        let parent_q = world
            .get_component_by_id_as::<TransformComponent>(joints[index].parent_id)
            .map(|transform| mat_to_quat(transform.transform.matrix_world))
            .unwrap_or([0.0, 0.0, 0.0, 1.0]);
        let tail = vec3_add(
            head,
            vec3_scale(
                quat_rotate_vec3(parent_q, joints[index].rest_direction),
                length,
            ),
        );
        lengths.push(length);
        current.push(tail);
    }
    Ok((
        BoundChain {
            joints,
            previous: current.clone(),
            current,
            lengths,
            accumulator: 0.0,
            enabled: chain.enabled,
            gltf: gltf_id,
            hit_radius: chain.hit_radius,
            colliders,
        },
        warnings,
    ))
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

fn simulate_step(world: &World, state: &mut BoundChain) {
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
        let rest_tail = if index + 1 < state.joints.len() {
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
    if state.colliders.is_empty() {
        return;
    }
    let hit_scale = world
        .parent_of(state.gltf)
        .map(|anchor| world_max_scale(world, anchor))
        .unwrap_or(1.0);
    let hit_radius = state.hit_radius * hit_scale;
    for _ in 0..2 {
        for index in 0..state.joints.len() {
            let head = if index == 0 {
                pos(world, state.joints[index].id).unwrap_or(state.current[index])
            } else {
                state.current[index - 1]
            };
            let mut endpoint = state.current[index];
            for collider in &state.colliders {
                let Some(center) = pos(world, collider.target) else {
                    continue;
                };
                let radius = collider.radius * world_max_scale(world, collider.target) + hit_radius;
                let delta = vec3_sub(endpoint, center);
                let distance = vec3_len(delta);
                if distance < radius {
                    endpoint = exclude_sphere_at_length(
                        head,
                        endpoint,
                        state.lengths[index],
                        center,
                        radius,
                    );
                }
            }
            let delta = vec3_sub(endpoint, head);
            let direction = if vec3_len(delta) > f32::EPSILON {
                vec3_normalize(delta)
            } else {
                [1.0, 0.0, 0.0]
            };
            state.current[index] = vec3_add(head, vec3_scale(direction, state.lengths[index]));
        }
    }
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

fn apply_rotations(world: &mut World, state: &BoundChain) -> f32 {
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
        let rotation = quat_mul(correction, joint.rest_rotation);
        previous_joint_world_q = Some(quat_mul(parent_q, rotation));
        if let Some(transform) = world.get_component_by_id_as_mut::<TransformComponent>(joint.id) {
            transform.transform.rotation = rotation;
            transform.transform.recompute_model();
        }
    }
    max_correction
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        world: World,
        system: SecondaryMotionSystem,
        root: ComponentId,
        chain: ComponentId,
        first_config: ComponentId,
        imported: Vec<ComponentId>,
    }

    fn fixture() -> Fixture {
        let mut world = World::default();
        let anchor = world.add_component(TransformComponent::new());
        let gltf = world.add_component(GLTFComponent::new("retained-test.glb"));
        let first =
            world.add_component_boxed_named("retained_first", Box::new(TransformComponent::new()));
        let second = world.add_component_boxed_named(
            "retained_second",
            Box::new(TransformComponent::new().with_position(0.0, 1.0, 0.0)),
        );
        let first_rest = world.add_component(BoneRestPoseComponent::new(
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
            [1.0; 3],
        ));
        let second_rest = world.add_component(BoneRestPoseComponent::new(
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
            [1.0; 3],
        ));
        let root = world.add_component(SecondaryMotionComponent::new());
        let chain = world.add_component(SpringBoneComponent::new("retained"));
        let first_config = world.add_component(SpringJointComponent::query("#retained_first"));
        let second_config = world.add_component(SpringJointComponent::query("#retained_second"));

        world.add_child(anchor, gltf).unwrap();
        world.add_child(gltf, first).unwrap();
        world.add_child(first, first_rest).unwrap();
        world.add_child(first, second).unwrap();
        world.add_child(second, second_rest).unwrap();
        world.add_child(gltf, root).unwrap();
        world.add_child(root, chain).unwrap();
        world.add_child(chain, first_config).unwrap();
        world.add_child(chain, second_config).unwrap();
        let component = world
            .get_component_by_id_as_mut::<GLTFComponent>(gltf)
            .unwrap();
        component.spawned_node_transforms = vec![first, second];
        component.armature_joint_transforms = vec![first, second];

        Fixture {
            world,
            system: SecondaryMotionSystem::default(),
            root,
            chain,
            first_config,
            imported: vec![first, second],
        }
    }

    fn automatic_fixture() -> (World, ComponentId, ComponentId, ComponentId, ComponentId) {
        let mut fixture = fixture();
        let gltf = nearest_gltf(&fixture.world, fixture.root).unwrap();
        fixture
            .world
            .get_component_by_id_as_mut::<SpringBoneComponent>(fixture.chain)
            .unwrap()
            .root = Some(ComponentRef::Query("#retained_first".into()));
        let explicit_configs: Vec<_> = fixture
            .world
            .children_of(fixture.chain)
            .iter()
            .copied()
            .filter(|id| {
                fixture
                    .world
                    .get_component_by_id_as::<SpringJointComponent>(*id)
                    .is_some()
            })
            .collect();
        for config in explicit_configs {
            fixture.world.detach_from_parent(config);
        }
        // Helpers below a joint must not participate in automatic skin topology.
        let collider = fixture.world.add_component_boxed_named(
            "retained_first_collider",
            Box::new(TransformComponent::new()),
        );
        fixture
            .world
            .add_child(fixture.imported[0], collider)
            .unwrap();
        (
            fixture.world,
            gltf,
            fixture.chain,
            fixture.imported[0],
            fixture.imported[1],
        )
    }

    #[test]
    fn automatic_chain_follows_skin_descendants_and_ignores_helpers() {
        let (world, gltf, chain, first, second) = automatic_fixture();
        let (bound, _) = build_chain(&world, gltf, chain, &[]).unwrap();
        assert_eq!(
            bound
                .joints
                .iter()
                .map(|joint| joint.id)
                .collect::<Vec<_>>(),
            vec![first, second]
        );
        assert!(bound.colliders.is_empty());
    }

    #[test]
    fn automatic_leaf_chain_uses_virtual_endpoint_and_chain_tuning() {
        let (mut world, gltf, chain, _first, second) = automatic_fixture();
        {
            let component = world
                .get_component_by_id_as_mut::<SpringBoneComponent>(chain)
                .unwrap();
            component.root = Some(ComponentRef::Query("#retained_second".into()));
            component.virtual_end_length_ratio = Some(1.0);
            component.stiffness = 2.0;
            component.drag_force = 0.35;
            component.gravity_power = 3.0;
        }
        let (bound, _) = build_chain(&world, gltf, chain, &[]).unwrap();
        assert_eq!(bound.joints.len(), 1);
        assert_eq!(bound.joints[0].id, second);
        assert_eq!(bound.joints[0].stiffness, 2.0);
        assert_eq!(bound.joints[0].drag, 0.35);
        assert_eq!(bound.joints[0].gravity, [0.0, -3.0, 0.0]);
    }

    #[test]
    fn explicit_collider_queries_are_gltf_scoped_deduplicated_and_skip_bad_targets() {
        let (mut world, gltf, chain, first, _second) = automatic_fixture();
        let config = world.add_component_boxed_named(
            "explicit_group",
            Box::new(SpringColliderComponent::spheres(
                vec![
                    ComponentRef::Query("#retained_first".into()),
                    ComponentRef::Query("#retained_first".into()),
                    ComponentRef::Query("#missing_target".into()),
                ],
                0.25,
            )),
        );
        world.add_child(gltf, config).unwrap();
        let config_guid = world.get_component_record(config).unwrap().guid;
        {
            let spring = world
                .get_component_by_id_as_mut::<SpringBoneComponent>(chain)
                .unwrap();
            spring.colliders = vec![
                ComponentRef::Query("#explicit_group".into()),
                ComponentRef::Guid(config_guid),
                ComponentRef::Query("#missing_group".into()),
            ];
            spring.hit_radius = 0.05;
        }
        let (bound, warnings) = build_chain(&world, gltf, chain, &[]).unwrap();
        assert_eq!(bound.colliders.len(), 1);
        assert_eq!(bound.colliders[0].target, first);
        assert_eq!(bound.hit_radius, 0.05);
        assert_eq!(warnings.len(), 2);

        let other_anchor = world.add_component(TransformComponent::new());
        let other_gltf = world.add_component(GLTFComponent::new("other.glb"));
        let outside = world.add_component_boxed_named(
            "outside_group",
            Box::new(SpringColliderComponent::sphere(
                ComponentRef::Query("#retained_first".into()),
                1.0,
            )),
        );
        world.add_child(other_anchor, other_gltf).unwrap();
        world.add_child(other_gltf, outside).unwrap();
        let outside_guid = world.get_component_record(outside).unwrap().guid;
        world
            .get_component_by_id_as_mut::<SpringBoneComponent>(chain)
            .unwrap()
            .colliders = vec![ComponentRef::Guid(outside_guid)];
        let (bound, warnings) = build_chain(&world, gltf, chain, &[]).unwrap();
        assert!(bound.colliders.is_empty());
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn bound_snapshot_projects_only_resolved_solver_geometry() {
        let mut fixture = fixture();
        let gltf = nearest_gltf(&fixture.world, fixture.root).unwrap();
        let collider = fixture.world.add_component_boxed_named(
            "snapshot_collider",
            Box::new(SpringColliderComponent::sphere(
                ComponentRef::Query("#retained_first".into()),
                0.25,
            )),
        );
        fixture.world.add_child(gltf, collider).unwrap();
        let collider_guid = fixture.world.get_component_record(collider).unwrap().guid;
        {
            let chain = fixture
                .world
                .get_component_by_id_as_mut::<SpringBoneComponent>(fixture.chain)
                .unwrap();
            chain.colliders = vec![ComponentRef::Guid(collider_guid)];
            chain.hit_radius = 0.05;
        }
        fixture.system.register(&fixture.world, fixture.root);

        let snapshot = fixture.system.bound_snapshot(&fixture.world);
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].gltf, gltf);
        assert_eq!(snapshot[0].chain, fixture.chain);
        assert!(snapshot[0].enabled);
        assert_eq!(snapshot[0].hit_radius, 0.05);
        assert_eq!(snapshot[0].segments.len(), 2);
        assert_eq!(snapshot[0].colliders.len(), 1);
        assert_eq!(snapshot[0].colliders[0].config, collider);
        assert_eq!(snapshot[0].colliders[0].target, fixture.imported[0]);
        assert_eq!(snapshot[0].colliders[0].scaled_base_radius, 0.25);
    }

    #[test]
    fn sphere_exclusion_adds_hit_radius_and_preserves_segment_length() {
        let head = [0.0, 0.0, 0.0];
        let center = [0.0, 0.75, 0.0];
        let endpoint = [0.0, 1.0, 0.0];
        let projected = exclude_sphere_at_length(head, endpoint, 1.0, center, 0.5 + 0.1);
        assert!((vec3_len(vec3_sub(projected, head)) - 1.0).abs() < 1e-5);
        assert!((vec3_len(vec3_sub(projected, center)) - 0.6).abs() < 1e-5);
    }

    #[test]
    fn coincident_sphere_center_has_a_deterministic_finite_result() {
        let first = exclude_sphere_at_length([0.0; 3], [0.0; 3], 1.0, [0.0; 3], 0.5);
        let second = exclude_sphere_at_length([0.0; 3], [0.0; 3], 1.0, [0.0; 3], 0.5);
        assert_eq!(first, second);
        assert_eq!(first, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn collider_target_topology_and_configuration_changes_rebind_the_chain() {
        let mut fixture = fixture();
        let gltf = nearest_gltf(&fixture.world, fixture.root).unwrap();
        let config = fixture.world.add_component_boxed_named(
            "live_group",
            Box::new(SpringColliderComponent::sphere(
                ComponentRef::Query("#retained_first".into()),
                0.25,
            )),
        );
        fixture.world.add_child(gltf, config).unwrap();
        let guid = fixture.world.get_component_record(config).unwrap().guid;
        fixture
            .world
            .get_component_by_id_as_mut::<SpringBoneComponent>(fixture.chain)
            .unwrap()
            .colliders = vec![ComponentRef::Guid(guid)];
        fixture.system.register(&fixture.world, fixture.root);

        let before = fixture.system.discovery_counts().0;
        fixture
            .system
            .topology_changed(&fixture.world, fixture.imported[0]);
        assert_eq!(fixture.system.discovery_counts().0, before + 1);

        fixture
            .world
            .get_component_by_id_as_mut::<SpringColliderComponent>(config)
            .unwrap()
            .radius = 0.5;
        fixture.system.configuration_changed(&fixture.world, config);
        assert_eq!(fixture.system.discovery_counts().0, before + 2);
    }

    #[test]
    fn automatic_chain_rejects_skin_branches_with_diagnostic() {
        let (mut world, gltf, chain, first, _second) = automatic_fixture();
        let branch =
            world.add_component_boxed_named("retained_branch", Box::new(TransformComponent::new()));
        let rest = world.add_component(BoneRestPoseComponent::new(
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
            [1.0; 3],
        ));
        world.add_child(first, branch).unwrap();
        world.add_child(branch, rest).unwrap();
        {
            let component = world
                .get_component_by_id_as_mut::<GLTFComponent>(gltf)
                .unwrap();
            component.spawned_node_transforms.push(branch);
            component.armature_joint_transforms.push(branch);
        }
        let error = build_chain(&world, gltf, chain, &[]).unwrap_err();
        assert!(error.message.contains("branches"), "{}", error.message);
        assert!(
            error.message.contains("retained_branch"),
            "{}",
            error.message
        );
    }

    #[test]
    fn automatic_chain_rebinds_after_skin_topology_changes() {
        let (mut world, gltf, chain, first, _second) = automatic_fixture();
        let root = world.parent_of(chain).unwrap();
        let mut system = SecondaryMotionSystem::default();
        system.register(&world, root);
        assert_eq!(system.runtime_counts(), (1, 1, 1, 0, 0));

        let branch =
            world.add_component_boxed_named("late_branch", Box::new(TransformComponent::new()));
        let rest = world.add_component(BoneRestPoseComponent::new(
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
            [1.0; 3],
        ));
        world.add_child(first, branch).unwrap();
        world.add_child(branch, rest).unwrap();
        {
            let component = world
                .get_component_by_id_as_mut::<GLTFComponent>(gltf)
                .unwrap();
            component.spawned_node_transforms.push(branch);
            component.armature_joint_transforms.push(branch);
        }
        system.topology_changed(&world, first);
        assert_eq!(system.runtime_counts(), (1, 1, 0, 0, 1));

        world.detach_from_parent(branch);
        system.topology_changed(&world, first);
        assert_eq!(system.runtime_counts(), (1, 1, 1, 0, 0));
    }

    #[test]
    fn registration_is_order_independent_and_idempotent() {
        let mut fixture = fixture();
        fixture
            .system
            .register(&fixture.world, fixture.first_config);
        assert_eq!(fixture.system.runtime_counts(), (1, 1, 1, 0, 0));
        let before = fixture.system.discovery_counts();
        fixture.system.register(&fixture.world, fixture.root);
        fixture.system.register(&fixture.world, fixture.chain);
        fixture
            .system
            .register(&fixture.world, fixture.first_config);
        assert_eq!(fixture.system.discovery_counts(), before);
    }

    #[test]
    fn steady_state_tick_does_not_retry_binding_or_discover_topology() {
        let mut fixture = fixture();
        fixture.system.register(&fixture.world, fixture.root);
        let before = fixture.system.discovery_counts();
        for _ in 0..256 {
            fixture.world.add_component(TransformComponent::new());
        }
        for _ in 0..8 {
            fixture.system.tick(&mut fixture.world, STEP);
        }
        assert_eq!(fixture.system.discovery_counts(), before);
        assert_eq!(fixture.system.runtime_counts(), (1, 1, 1, 0, 0));
    }

    #[test]
    fn configuration_change_rebinds_only_the_owning_chain() {
        let mut fixture = fixture();
        fixture.system.register(&fixture.world, fixture.root);
        let before = fixture.system.discovery_counts().0;
        fixture
            .world
            .get_component_by_id_as_mut::<SpringJointComponent>(fixture.first_config)
            .unwrap()
            .stiffness = 4.0;
        fixture
            .system
            .configuration_changed(&fixture.world, fixture.first_config);
        assert_eq!(fixture.system.discovery_counts().0, before + 1);
        assert_eq!(fixture.system.runtime_counts(), (1, 1, 1, 0, 0));
    }

    #[test]
    fn reparenting_and_subtree_cleanup_update_retained_ownership() {
        let mut fixture = fixture();
        fixture.system.register(&fixture.world, fixture.root);
        fixture.world.detach_from_parent(fixture.chain);
        fixture
            .system
            .topology_changed(&fixture.world, fixture.chain);
        assert_eq!(fixture.system.runtime_counts(), (1, 0, 0, 0, 1));
        fixture
            .world
            .add_child(fixture.root, fixture.chain)
            .unwrap();
        fixture
            .system
            .topology_changed(&fixture.world, fixture.chain);
        assert_eq!(fixture.system.runtime_counts(), (1, 1, 1, 0, 0));

        fixture
            .system
            .component_removed(&fixture.world, fixture.first_config);
        fixture
            .system
            .component_removed(&fixture.world, fixture.chain);
        fixture
            .system
            .component_removed(&fixture.world, fixture.root);
        for imported in fixture.imported {
            fixture.system.component_removed(&fixture.world, imported);
        }
        assert_eq!(fixture.system.runtime_counts(), (0, 0, 0, 0, 0));
    }

    #[test]
    fn gltf_readiness_retries_a_waiting_chain_without_frame_polling() {
        let mut fixture = fixture();
        let gltf = nearest_gltf(&fixture.world, fixture.root).unwrap();
        fixture
            .world
            .get_component_by_id_as_mut::<GLTFComponent>(gltf)
            .unwrap()
            .spawned_node_transforms
            .clear();
        fixture.system.register(&fixture.world, fixture.root);
        assert_eq!(fixture.system.runtime_counts(), (1, 1, 0, 1, 0));
        let before = fixture.system.discovery_counts().0;
        fixture.system.tick(&mut fixture.world, STEP);
        assert_eq!(fixture.system.discovery_counts().0, before);

        fixture
            .world
            .get_component_by_id_as_mut::<GLTFComponent>(gltf)
            .unwrap()
            .spawned_node_transforms = fixture.imported.clone();
        fixture.system.gltf_initialized(&fixture.world, gltf);
        assert_eq!(fixture.system.runtime_counts(), (1, 1, 1, 0, 0));
    }

    #[test]
    fn overlapping_chain_retries_when_the_owner_is_removed() {
        let mut fixture = fixture();
        fixture.system.register(&fixture.world, fixture.root);
        let second_chain = fixture
            .world
            .add_component(SpringBoneComponent::new("overlap"));
        let first = fixture
            .world
            .add_component(SpringJointComponent::query("#retained_first"));
        let second = fixture
            .world
            .add_component(SpringJointComponent::query("#retained_second"));
        fixture.world.add_child(fixture.root, second_chain).unwrap();
        fixture.world.add_child(second_chain, first).unwrap();
        fixture.world.add_child(second_chain, second).unwrap();
        fixture.system.register(&fixture.world, second_chain);
        assert_eq!(fixture.system.runtime_counts(), (1, 2, 1, 0, 1));

        fixture
            .system
            .component_removed(&fixture.world, fixture.chain);
        assert_eq!(fixture.system.runtime_counts(), (1, 1, 1, 0, 0));
    }

    #[test]
    fn imported_transform_removal_invalidates_only_its_bound_chain() {
        let mut fixture = fixture();
        fixture.system.register(&fixture.world, fixture.root);
        fixture
            .system
            .component_removed(&fixture.world, fixture.imported[0]);
        assert_eq!(fixture.system.runtime_counts(), (1, 1, 0, 1, 0));
        let before = fixture.system.discovery_counts();
        fixture.system.tick(&mut fixture.world, STEP);
        assert_eq!(fixture.system.discovery_counts(), before);
    }

    #[test]
    fn joint_removal_reordering_and_readdition_rebind_the_chain() {
        let mut fixture = fixture();
        fixture.system.register(&fixture.world, fixture.root);
        let second_config = fixture.world.children_of(fixture.chain)[1];

        fixture.world.detach_from_parent(second_config);
        fixture
            .system
            .topology_changed(&fixture.world, second_config);
        assert_eq!(fixture.system.runtime_counts(), (1, 1, 0, 0, 1));
        fixture
            .world
            .get_component_by_id_as_mut::<SpringBoneComponent>(fixture.chain)
            .unwrap()
            .virtual_end_length_ratio = Some(1.0);
        fixture
            .system
            .configuration_changed(&fixture.world, fixture.chain);
        assert_eq!(fixture.system.runtime_counts(), (1, 1, 1, 0, 0));

        fixture
            .world
            .add_child(fixture.chain, second_config)
            .unwrap();
        fixture
            .system
            .topology_changed(&fixture.world, second_config);
        assert_eq!(fixture.system.runtime_counts(), (1, 1, 1, 0, 0));
        fixture.world.detach_from_parent(fixture.first_config);
        fixture
            .world
            .add_child(fixture.chain, fixture.first_config)
            .unwrap();
        fixture
            .system
            .topology_changed(&fixture.world, fixture.first_config);
        assert_eq!(fixture.system.runtime_counts(), (1, 1, 0, 0, 1));
    }

    #[test]
    fn root_reparent_transfers_gltf_ownership_and_targets_respawn() {
        let mut fixture = fixture();
        fixture.system.register(&fixture.world, fixture.root);
        let old_gltf = nearest_gltf(&fixture.world, fixture.root).unwrap();
        let anchor = fixture.world.parent_of(old_gltf).unwrap();
        let new_gltf = fixture
            .world
            .add_component(GLTFComponent::new("retained-test-respawn.glb"));
        fixture.world.add_child(anchor, new_gltf).unwrap();
        fixture
            .world
            .get_component_by_id_as_mut::<GLTFComponent>(new_gltf)
            .unwrap()
            .spawned_node_transforms = fixture.imported.clone();
        fixture.world.detach_from_parent(fixture.root);
        fixture.world.add_child(new_gltf, fixture.root).unwrap();
        fixture
            .system
            .topology_changed(&fixture.world, fixture.root);
        assert_eq!(fixture.system.runtime_counts(), (1, 1, 1, 0, 0));

        let before = fixture.system.discovery_counts().0;
        fixture.system.gltf_initialized(&fixture.world, old_gltf);
        assert_eq!(fixture.system.discovery_counts().0, before);
        fixture.system.gltf_initialized(&fixture.world, new_gltf);
        assert_eq!(fixture.system.discovery_counts().0, before + 1);
    }
}
