//! Cold-path chain scope resolution, discovery, and rest-pose binding.
use super::*;

impl SecondaryMotionSystem {
    pub(super) fn reconcile_root(&mut self, world: &World, root: ComponentId) {
        self.profile.topology_discoveries += 1;
        let new_gltf = nearest_armature_owner(world, root);
        let old_gltf = self
            .roots
            .get(&root)
            .and_then(|runtime| runtime.armature_owner);
        if old_gltf != new_gltf {
            if let Some(old) = old_gltf
                && let Some(roots) = self.owner_roots.get_mut(&old)
            {
                roots.remove(&root);
                if roots.is_empty() {
                    self.owner_roots.remove(&old);
                }
            }
            if let Some(new) = new_gltf {
                self.owner_roots.entry(new).or_default().insert(root);
            }
            self.roots.get_mut(&root).unwrap().armature_owner = new_gltf;
        }
        let chains: Vec<_> = self.roots[&root].children.iter().copied().collect();
        for chain in chains {
            self.bind_chain(world, chain);
        }
    }

    pub(super) fn reconcile_chain(&mut self, world: &World, chain: ComponentId) {
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

    pub(super) fn retry_invalid_children(
        &mut self,
        world: &World,
        root: ComponentId,
        except: ComponentId,
    ) {
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

    pub(super) fn bind_chain(&mut self, world: &World, chain: ComponentId) {
        let started = Instant::now();
        self.profile.binding_attempts += 1;
        self.release_resolved_ownership(chain);
        let Some(root) = self.chains.get(&chain).and_then(|runtime| runtime.root) else {
            return;
        };
        let Some(armature_owner) = self
            .roots
            .get(&root)
            .and_then(|runtime| runtime.armature_owner)
        else {
            self.chains.get_mut(&chain).unwrap().status = ChainStatus::WaitingForDependencies(
                "SecondaryMotion has no owning Armature or GLTF".into(),
            );
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
        match build_chain(world, armature_owner, chain, &joint_config_ids) {
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
}

pub(super) fn nearest_armature_owner(world: &World, mut id: ComponentId) -> Option<ComponentId> {
    for _ in 0..64 {
        id = world.parent_of(id)?;
        if world.get_component_by_id_as::<GLTFComponent>(id).is_some()
            || world
                .get_component_by_id_as::<ArmatureComponent>(id)
                .is_some()
        {
            return Some(id);
        }
    }
    None
}

pub(super) fn descendant(world: &World, mut node: ComponentId, ancestor: ComponentId) -> bool {
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

pub(super) fn pos(world: &World, id: ComponentId) -> Option<[f32; 3]> {
    let matrix = world
        .get_component_by_id_as::<TransformComponent>(id)?
        .transform
        .matrix_world;
    Some([matrix[3][0], matrix[3][1], matrix[3][2]])
}

pub(super) fn rest(world: &World, id: ComponentId) -> Option<BoneRestPoseComponent> {
    world.children_of(id).iter().find_map(|child| {
        world
            .get_component_by_id_as::<BoneRestPoseComponent>(*child)
            .copied()
    })
}

pub(super) fn ref_surface(reference: &ComponentRef) -> String {
    match reference {
        ComponentRef::Guid(guid) => format!("@uuid:{guid}"),
        ComponentRef::Query(query) => query.clone(),
    }
}

#[derive(Debug)]
pub(super) struct BindFailure {
    pub(super) waiting: bool,
    pub(super) message: String,
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

pub(super) fn resolve_in_armature(
    world: &World,
    gltf_id: ComponentId,
    reference: &ComponentRef,
) -> Result<ComponentId, BindFailure> {
    if world
        .get_component_by_id_as::<ArmatureComponent>(gltf_id)
        .is_some()
    {
        let root = super::super::armature_scope::joint_root(world, gltf_id)
            .map_err(BindFailure::waiting)?;
        let mut candidates = Vec::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if world
                .get_component_by_id_as::<TransformComponent>(id)
                .is_some()
            {
                candidates.push(id);
            }
            stack.extend(world.children_of(id).iter().rev().copied());
        }
        let id = match reference {
            ComponentRef::Guid(guid) => world.component_id_by_guid(*guid),
            ComponentRef::Query(query) => {
                let matches: Vec<_> = candidates
                    .iter()
                    .copied()
                    .filter(|id| world.component_matches_selector(*id, query))
                    .collect();
                if matches.len() != 1 {
                    return Err(BindFailure::invalid(format!(
                        "query '{query}' matched {} transforms in the Armature joint tree",
                        matches.len()
                    )));
                }
                matches.first().copied()
            }
        }
        .ok_or_else(|| BindFailure::waiting("joint reference is not ready"))?;
        if !candidates.contains(&id) {
            return Err(BindFailure::invalid(
                "reference is outside the Armature joint tree",
            ));
        }
        return Ok(id);
    }
    let armature_owner = world
        .get_component_by_id_as::<GLTFComponent>(gltf_id)
        .ok_or_else(|| BindFailure::waiting("owning GLTF disappeared"))?;
    let instance_nodes: HashSet<_> = armature_owner
        .spawned_node_transforms
        .iter()
        .copied()
        .collect();
    if instance_nodes.is_empty() {
        return Err(BindFailure::waiting(
            "owning GLTF has not spawned its node transforms yet",
        ));
    }
    let anchor = owner_anchor(world, gltf_id)
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

fn resolve_collider_config(
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
        || nearest_armature_owner(world, id) != Some(gltf_id)
    {
        return Err(format!(
            "collider configuration '{}' resolved outside the owning GLTF instance or is not a SpringCollider",
            ref_surface(reference)
        ));
    }
    Ok(id)
}

pub(super) fn world_max_scale(world: &World, id: ComponentId) -> f32 {
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

pub(super) fn build_chain(
    world: &World,
    gltf_id: ComponentId,
    chain_id: ComponentId,
    joint_config_ids: &[ComponentId],
) -> Result<(BoundChain, Vec<String>), BindFailure> {
    if let Some(anchor) = owner_anchor(world, gltf_id)
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
        let config_id = match resolve_collider_config(world, gltf_id, collider_ref) {
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
            match resolve_in_armature(world, gltf_id, target_ref) {
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
        resolve_in_armature(world, gltf_id, center).map_err(|failure| BindFailure {
            waiting: failure.waiting,
            message: format!("center {}", failure.message),
        })?;
    }
    let auto_ids = if joint_config_ids.is_empty() {
        let root_ref = chain
            .root
            .as_ref()
            .ok_or_else(|| BindFailure::invalid("has no SpringJoint children"))?;
        let root_id = resolve_in_armature(world, gltf_id, root_ref)?;
        let skin_joints = skin_joints(world, gltf_id);
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
                resolve_in_armature(world, gltf_id, &config.node)?,
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
        let mut parent = world.parent_of(id);
        let parent_id = loop {
            let id = parent.ok_or_else(|| {
                BindFailure::waiting(format!("joint '{surface}' has no transform parent"))
            })?;
            if world
                .get_component_by_id_as::<TransformComponent>(id)
                .is_some()
            {
                break id;
            }
            parent = world.parent_of(id);
        };
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
    if chain.allow_stretch {
        stretch::validate(world, gltf_id, chain, &joints).map_err(BindFailure::invalid)?;
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
    let instance_scale = owner_anchor(world, gltf_id)
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
            allow_stretch: chain.allow_stretch,
            rest_targets: rest_targets::RestTargets::bind(world, gltf_id, &joints),
            joints,
            previous: current.clone(),
            current,
            lengths,
            accumulator: 0.0,
            enabled: chain.enabled,
            parked: false,
            armature_owner: gltf_id,
            hit_radius: chain.hit_radius,
            colliders,
        },
        warnings,
    ))
}

pub(super) fn skin_joints(world: &World, owner: ComponentId) -> HashSet<ComponentId> {
    if let Some(armature_owner) = world.get_component_by_id_as::<GLTFComponent>(owner) {
        armature_owner
            .armature_joint_transforms
            .iter()
            .copied()
            .collect()
    } else {
        super::super::armature_scope::joint_transforms(world, owner)
            .unwrap_or_default()
            .into_iter()
            .collect()
    }
}

pub(super) fn owner_anchor(world: &World, owner: ComponentId) -> Option<ComponentId> {
    if world
        .get_component_by_id_as::<ArmatureComponent>(owner)
        .is_some()
    {
        super::super::armature_scope::joint_root(world, owner).ok()
    } else {
        world.parent_of(owner)
    }
}

pub(super) fn owns_frame(world: &World, owner: ComponentId, frame: ComponentId) -> bool {
    if let Some(gltf) = world.get_component_by_id_as::<GLTFComponent>(owner) {
        gltf.spawned_node_transforms.contains(&frame)
    } else {
        super::super::armature_scope::joint_root(world, owner)
            .is_ok_and(|root| descendant(world, frame, root))
    }
}
