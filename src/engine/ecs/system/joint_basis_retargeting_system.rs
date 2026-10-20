use crate::engine::ecs::component::{
    BoneRestPoseComponent, ComponentRef, GLTFComponent, JointRetargetBasisComponent,
    TransformComponent,
};
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, RxWorld, Signal, SignalEmitter, SignalKind, World,
};
use crate::engine::graphics::primitives::Transform;
use crate::utils::math::{
    mat4_identity, mat4_inverse, mat4_mul, mat4_mul_vec4, vec3_cross, vec3_dot, vec3_len,
    vec3_normalize, vec3_scale, vec3_sub,
};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LandmarkDirection {
    pub start: ComponentId,
    pub end: ComponentId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RetargetBasisDefinition {
    pub target: ComponentId,
    pub forward: LandmarkDirection,
    pub up: LandmarkDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetargetBasisProvenance {
    pub source: ComponentId,
    pub source_label: String,
    pub owning_gltf: ComponentId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRetargetBasis {
    pub target: ComponentId,
    pub forward: [f32; 3],
    pub up: [f32; 3],
    pub right: [f32; 3],
    /// Rotation-only matrix whose columns map canonical +X/+Y/+Z into target rest space.
    pub canonical_to_target_rest: [[f32; 4]; 4],
    pub target_rest_to_canonical: [[f32; 4]; 4],
    pub generation: u64,
    pub provenance: RetargetBasisProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetargetBasisStatus {
    WaitingForGltf,
    Ready,
    Invalid(String),
    ConflictingDefinition { sources: Vec<ComponentId> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetargetBasisDiagnosticSnapshot {
    pub source: ComponentId,
    pub target: Option<ComponentId>,
    pub owning_gltf: Option<ComponentId>,
    pub status: RetargetBasisStatus,
}

#[derive(Debug, Clone)]
struct SourceEntry {
    owner: Option<ComponentId>,
    definition: Option<RetargetBasisDefinition>,
    status: RetargetBasisStatus,
    label: String,
}

#[derive(Debug, Default)]
pub struct JointBasisRetargetingSystem {
    sources: HashMap<ComponentId, SourceEntry>,
    target_sources: HashMap<ComponentId, HashSet<ComponentId>>,
    owner_sources: HashMap<ComponentId, HashSet<ComponentId>>,
    dependency_sources: HashMap<ComponentId, HashSet<ComponentId>>,
    published: HashMap<ComponentId, ResolvedRetargetBasis>,
    generations: HashMap<ComponentId, u64>,
}

impl JointBasisRetargetingSystem {
    pub fn install_handlers(rx: &mut RxWorld) {
        rx.add_global_handler_named(
            SignalKind::GltfInitialized,
            Some("joint_retarget_basis_gltf_initialized".into()),
            gltf_initialized_handler,
        );
    }

    pub fn basis_for(&self, target: ComponentId) -> Option<&ResolvedRetargetBasis> {
        self.published.get(&target)
    }

    pub fn status_for(&self, target: ComponentId) -> Option<RetargetBasisStatus> {
        let sources = self.target_sources.get(&target)?;
        if sources.len() > 1 {
            let mut sources: Vec<_> = sources.iter().copied().collect();
            sources.sort();
            return Some(RetargetBasisStatus::ConflictingDefinition { sources });
        }
        sources
            .iter()
            .next()
            .and_then(|source| self.status_for_source(*source))
    }

    pub fn status_for_source(&self, source: ComponentId) -> Option<RetargetBasisStatus> {
        self.sources.get(&source).map(|entry| entry.status.clone())
    }

    pub fn diagnostic_snapshots(&self) -> Vec<RetargetBasisDiagnosticSnapshot> {
        let mut result: Vec<_> = self
            .sources
            .iter()
            .map(|(&source, entry)| RetargetBasisDiagnosticSnapshot {
                source,
                target: entry.definition.map(|definition| definition.target),
                owning_gltf: entry.owner,
                status: entry.status.clone(),
            })
            .collect();
        result.sort_by_key(|entry| entry.source);
        result
    }

    pub fn register_component(&mut self, world: &World, source: ComponentId) {
        let Some(component) = world.get_component_by_id_as::<JointRetargetBasisComponent>(source)
        else {
            return;
        };
        self.remove_source_indexes(source);
        let label = world.component_label(source).unwrap_or_default().to_owned();
        let Some(owner) = nearest_ancestor_gltf(world, source) else {
            self.sources.insert(
                source,
                SourceEntry {
                    owner: None,
                    definition: None,
                    status: RetargetBasisStatus::Invalid(
                        "JointRetargetBasis must be a descendant of exactly one GLTF".into(),
                    ),
                    label,
                },
            );
            return;
        };
        self.owner_sources.entry(owner).or_default().insert(source);
        let Some(gltf) = world.get_component_by_id_as::<GLTFComponent>(owner) else {
            return;
        };
        if gltf.armature_joint_transforms.is_empty() {
            self.sources.insert(
                source,
                SourceEntry {
                    owner: Some(owner),
                    definition: None,
                    status: RetargetBasisStatus::WaitingForGltf,
                    label,
                },
            );
            return;
        }
        let refs = [
            &component.target,
            &component.forward_start,
            &component.forward_end,
            &component.up_start,
            &component.up_end,
        ];
        let mut ids = Vec::with_capacity(5);
        for reference in refs {
            match resolve_in_armature(world, gltf, reference) {
                Ok(id) => ids.push(id),
                Err(error) => {
                    self.sources.insert(
                        source,
                        SourceEntry {
                            owner: Some(owner),
                            definition: None,
                            status: RetargetBasisStatus::Invalid(error),
                            label,
                        },
                    );
                    return;
                }
            }
        }
        self.replace_definition_with_owner(
            world,
            source,
            owner,
            RetargetBasisDefinition {
                target: ids[0],
                forward: LandmarkDirection {
                    start: ids[1],
                    end: ids[2],
                },
                up: LandmarkDirection {
                    start: ids[3],
                    end: ids[4],
                },
            },
            label,
        );
    }

    pub fn replace_definition(
        &mut self,
        world: &World,
        source: ComponentId,
        definition: RetargetBasisDefinition,
    ) {
        let owner = self
            .sources
            .get(&source)
            .and_then(|entry| entry.owner)
            .or_else(|| owner_for_definition(world, definition));
        let old_target = self
            .sources
            .get(&source)
            .and_then(|entry| entry.definition.map(|definition| definition.target));
        self.remove_source_indexes(source);
        let label = world.component_label(source).unwrap_or_default().to_owned();
        let Some(owner) = owner else {
            self.sources.insert(
                source,
                SourceEntry {
                    owner: None,
                    definition: Some(definition),
                    status: RetargetBasisStatus::Invalid(
                        "definition members do not share an owning GLTF armature".into(),
                    ),
                    label,
                },
            );
            return;
        };
        self.replace_definition_with_owner(world, source, owner, definition, label);
        if old_target.is_some_and(|target| target != definition.target) {
            self.reconcile_target(world, old_target.unwrap());
        }
    }

    fn replace_definition_with_owner(
        &mut self,
        world: &World,
        source: ComponentId,
        owner: ComponentId,
        definition: RetargetBasisDefinition,
        label: String,
    ) {
        self.owner_sources.entry(owner).or_default().insert(source);
        self.target_sources
            .entry(definition.target)
            .or_default()
            .insert(source);
        for dependency in definition_members(definition) {
            self.dependency_sources
                .entry(dependency)
                .or_default()
                .insert(source);
        }
        self.sources.insert(
            source,
            SourceEntry {
                owner: Some(owner),
                definition: Some(definition),
                status: RetargetBasisStatus::WaitingForGltf,
                label,
            },
        );
        self.reconcile_target(world, definition.target);
    }

    pub fn remove_definition(&mut self, world: &World, source: ComponentId) {
        let target = self
            .sources
            .get(&source)
            .and_then(|entry| entry.definition.map(|d| d.target));
        self.remove_source_indexes(source);
        self.sources.remove(&source);
        if let Some(target) = target {
            self.reconcile_target(world, target);
        }
    }

    pub fn component_removed(&mut self, world: &World, component: ComponentId) {
        if self.sources.contains_key(&component) {
            self.remove_definition(world, component);
            return;
        }
        let mut affected = HashSet::new();
        if let Some(sources) = self.owner_sources.get(&component) {
            affected.extend(sources.iter().copied());
        }
        if let Some(sources) = self.dependency_sources.get(&component) {
            affected.extend(sources.iter().copied());
        }
        for source in affected {
            self.remove_definition(world, source);
        }
    }

    pub fn gltf_initialized(&mut self, world: &World, gltf: ComponentId) {
        let sources: Vec<_> = self
            .owner_sources
            .get(&gltf)
            .into_iter()
            .flatten()
            .copied()
            .collect();
        for source in sources {
            self.register_component(world, source);
        }
    }

    /// Explicit invalidation seam for a GLTF instance whose imported nodes are respawned.
    pub fn invalidate_gltf_generation(&mut self, world: &World, gltf: ComponentId) {
        let sources: Vec<_> = self
            .owner_sources
            .get(&gltf)
            .into_iter()
            .flatten()
            .copied()
            .collect();
        for source in sources {
            self.remove_source_indexes(source);
            let label = world.component_label(source).unwrap_or_default().to_owned();
            self.owner_sources.entry(gltf).or_default().insert(source);
            self.sources.insert(
                source,
                SourceEntry {
                    owner: Some(gltf),
                    definition: None,
                    status: RetargetBasisStatus::WaitingForGltf,
                    label,
                },
            );
        }
    }

    fn reconcile_target(&mut self, world: &World, target: ComponentId) {
        let sources: Vec<_> = self
            .target_sources
            .get(&target)
            .into_iter()
            .flatten()
            .copied()
            .collect();
        self.published.remove(&target);
        if sources.len() > 1 {
            let mut ordered = sources.clone();
            ordered.sort();
            for source in sources {
                if let Some(entry) = self.sources.get_mut(&source) {
                    entry.status = RetargetBasisStatus::ConflictingDefinition {
                        sources: ordered.clone(),
                    };
                }
            }
            return;
        }
        let Some(source) = sources.first().copied() else {
            return;
        };
        let Some(entry) = self.sources.get(&source).cloned() else {
            return;
        };
        let (Some(owner), Some(definition)) = (entry.owner, entry.definition) else {
            return;
        };
        match compute_basis(
            world,
            owner,
            definition,
            source,
            entry.label.clone(),
            self.next_generation(target),
        ) {
            Ok(basis) => {
                self.published.insert(target, basis);
                if let Some(entry) = self.sources.get_mut(&source) {
                    entry.status = RetargetBasisStatus::Ready;
                }
            }
            Err(error) => {
                if let Some(entry) = self.sources.get_mut(&source) {
                    entry.status = RetargetBasisStatus::Invalid(error);
                }
            }
        }
    }

    fn next_generation(&mut self, target: ComponentId) -> u64 {
        let value = self.generations.entry(target).or_default();
        *value = value.saturating_add(1);
        *value
    }

    fn remove_source_indexes(&mut self, source: ComponentId) {
        let Some(entry) = self.sources.get(&source).cloned() else {
            return;
        };
        if let Some(owner) = entry.owner {
            remove_index(&mut self.owner_sources, owner, source);
        }
        if let Some(definition) = entry.definition {
            remove_index(&mut self.target_sources, definition.target, source);
            self.published.remove(&definition.target);
            for dependency in definition_members(definition) {
                remove_index(&mut self.dependency_sources, dependency, source);
            }
        }
    }
}

fn remove_index(
    index: &mut HashMap<ComponentId, HashSet<ComponentId>>,
    key: ComponentId,
    source: ComponentId,
) {
    if let Some(values) = index.get_mut(&key) {
        values.remove(&source);
        if values.is_empty() {
            index.remove(&key);
        }
    }
}

fn definition_members(definition: RetargetBasisDefinition) -> [ComponentId; 5] {
    [
        definition.target,
        definition.forward.start,
        definition.forward.end,
        definition.up.start,
        definition.up.end,
    ]
}

fn nearest_ancestor_gltf(world: &World, mut id: ComponentId) -> Option<ComponentId> {
    for _ in 0..64 {
        id = world.parent_of(id)?;
        if world.get_component_by_id_as::<GLTFComponent>(id).is_some() {
            return Some(id);
        }
    }
    None
}

fn resolve_in_armature(
    world: &World,
    gltf: &GLTFComponent,
    reference: &ComponentRef,
) -> Result<ComponentId, String> {
    let matches: Vec<_> = gltf
        .armature_joint_transforms
        .iter()
        .copied()
        .filter(|id| match reference {
            ComponentRef::Guid(guid) => world.component_id_by_guid(*guid) == Some(*id),
            ComponentRef::Query(query) => world.component_matches_selector(*id, query),
        })
        .collect();
    if matches.len() == 1 {
        Ok(matches[0])
    } else {
        Err(format!(
            "reference {} matched {} joints in the owning GLTF armature (expected exactly one)",
            ref_surface(reference),
            matches.len()
        ))
    }
}

fn ref_surface(reference: &ComponentRef) -> String {
    match reference {
        ComponentRef::Guid(guid) => format!("@uuid:{guid}"),
        ComponentRef::Query(q) => q.clone(),
    }
}

fn owner_for_definition(world: &World, definition: RetargetBasisDefinition) -> Option<ComponentId> {
    world.all_components().find(|id| {
        world
            .get_component_by_id_as::<GLTFComponent>(*id)
            .is_some_and(|gltf| {
                definition_members(definition)
                    .iter()
                    .all(|member| gltf.armature_joint_transforms.contains(member))
            })
    })
}

fn rest_local(world: &World, id: ComponentId) -> Result<[[f32; 4]; 4], String> {
    let rest = world
        .children_of(id)
        .iter()
        .find_map(|child| world.get_component_by_id_as::<BoneRestPoseComponent>(*child));
    let mut transform = Transform::default();
    if let Some(rest) = rest {
        transform.translation = rest.translation;
        transform.rotation = rest.rotation;
        transform.scale = rest.scale;
    } else {
        return Err(format!("joint {id:?} has no immutable rest pose"));
    }
    transform.recompute_model();
    Ok(transform.model)
}

pub(crate) fn imported_rest_space_root(
    world: &World,
    owner: ComponentId,
    member: ComponentId,
) -> ComponentId {
    let mut member_ancestor = world.parent_of(member);
    while let Some(node) = member_ancestor {
        if node == owner {
            return owner;
        }
        member_ancestor = world.parent_of(node);
    }

    let mut current = world.parent_of(owner);
    while let Some(node) = current {
        if world
            .get_component_by_id_as::<TransformComponent>(node)
            .is_some()
        {
            return node;
        }
        current = world.parent_of(node);
    }
    // Hand-built armatures may place joints directly below GLTF without a transform host.
    owner
}

pub(crate) fn rest_model_from_root(
    world: &World,
    root: ComponentId,
    id: ComponentId,
) -> Result<[[f32; 4]; 4], String> {
    let mut chain = Vec::new();
    let mut current = Some(id);
    while let Some(node) = current {
        if node == root {
            chain.reverse();
            return chain.into_iter().try_fold(mat4_identity(), |model, joint| {
                Ok(mat4_mul(model, rest_local(world, joint)?))
            });
        }
        chain.push(node);
        current = world.parent_of(node);
    }
    Err("joint is not beneath the owning GLTF's rest-space root".into())
}

fn compute_basis(
    world: &World,
    owner: ComponentId,
    definition: RetargetBasisDefinition,
    source: ComponentId,
    label: String,
    generation: u64,
) -> Result<ResolvedRetargetBasis, String> {
    let gltf = world
        .get_component_by_id_as::<GLTFComponent>(owner)
        .ok_or("owning GLTF disappeared")?;
    if !definition_members(definition)
        .iter()
        .all(|id| gltf.armature_joint_transforms.contains(id))
    {
        return Err("definition contains a joint outside the owning GLTF armature".into());
    }
    let rest_root = imported_rest_space_root(world, owner, definition.target);
    let target_model = rest_model_from_root(world, rest_root, definition.target)?;
    let target_inverse = mat4_inverse(target_model).ok_or("target rest transform is singular")?;
    let position = |joint| -> Result<[f32; 3], String> {
        let model = rest_model_from_root(world, rest_root, joint)?;
        let p = mat4_mul_vec4(target_inverse, [model[3][0], model[3][1], model[3][2], 1.0]);
        if !p.iter().all(|v| v.is_finite()) {
            return Err("rest landmark produced a non-finite position".into());
        }
        Ok([p[0], p[1], p[2]])
    };
    let forward_raw = vec3_sub(
        position(definition.forward.end)?,
        position(definition.forward.start)?,
    );
    if vec3_len(forward_raw) <= 1e-6 {
        return Err("forward landmark direction has zero length".into());
    }
    let forward = vec3_normalize(forward_raw);
    let up_raw = vec3_sub(position(definition.up.end)?, position(definition.up.start)?);
    if vec3_len(up_raw) <= 1e-6 {
        return Err("up landmark direction has zero length".into());
    }
    let projected_up = vec3_sub(up_raw, vec3_scale(forward, vec3_dot(up_raw, forward)));
    if vec3_len(projected_up) <= 1e-6 {
        return Err("up landmark direction is collinear with forward".into());
    }
    let up = vec3_normalize(projected_up);
    let z = vec3_scale(forward, -1.0);
    let right = vec3_normalize(vec3_cross(up, z));
    let up = vec3_normalize(vec3_cross(z, right));
    let canonical_to_target_rest = [
        [right[0], right[1], right[2], 0.0],
        [up[0], up[1], up[2], 0.0],
        [z[0], z[1], z[2], 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let target_rest_to_canonical =
        mat4_inverse(canonical_to_target_rest).ok_or("derived basis is singular")?;
    Ok(ResolvedRetargetBasis {
        target: definition.target,
        forward,
        up,
        right,
        canonical_to_target_rest,
        target_rest_to_canonical,
        generation,
        provenance: RetargetBasisProvenance {
            source,
            source_label: label,
            owning_gltf: owner,
        },
    })
}

fn gltf_initialized_handler(_world: &mut World, emit: &mut dyn SignalEmitter, signal: &Signal) {
    if let Some(EventSignal::GltfInitialized { gltf, .. }) = signal.event.as_ref() {
        emit.push_intent_now(
            *gltf,
            IntentValue::JointRetargetBasisGltfInitialized {
                component_id: *gltf,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::system::{GLTFSystem, RenderableSystem, SkinnedMeshSystem};
    use crate::engine::graphics::VisualWorld;
    use crate::utils::math::mat4_mul_vec4;

    struct Fixture {
        world: World,
        gltf: ComponentId,
        target: ComponentId,
        forward_start: ComponentId,
        forward_end: ComponentId,
        up_start: ComponentId,
        up_end: ComponentId,
    }

    fn fixture() -> Fixture {
        let mut world = World::default();
        let gltf = world.add_component(GLTFComponent::new("fixture.glb"));
        let mut add_joint = |label: &str, position: [f32; 3]| {
            let joint = world.add_component_boxed_named(label, Box::new(TransformComponent::new()));
            let rest = world.add_component(BoneRestPoseComponent::new(
                position,
                [0.0, 0.0, 0.0, 1.0],
                [1.0; 3],
            ));
            world.add_child(gltf, joint).unwrap();
            world.add_child(joint, rest).unwrap();
            joint
        };
        let target = add_joint("hand", [2.0, 3.0, 4.0]);
        let forward_start = add_joint("middle1", [2.0, 3.0, 4.0]);
        let forward_end = add_joint("middle3", [2.0, 3.0, 2.0]);
        let up_start = add_joint("little1", [1.0, 3.0, 4.0]);
        let up_end = add_joint("index1", [3.0, 3.0, 4.0]);
        world
            .get_component_by_id_as_mut::<GLTFComponent>(gltf)
            .unwrap()
            .armature_joint_transforms = vec![target, forward_start, forward_end, up_start, up_end];
        Fixture {
            world,
            gltf,
            target,
            forward_start,
            forward_end,
            up_start,
            up_end,
        }
    }

    fn definition(f: &Fixture) -> RetargetBasisDefinition {
        RetargetBasisDefinition {
            target: f.target,
            forward: LandmarkDirection {
                start: f.forward_start,
                end: f.forward_end,
            },
            up: LandmarkDirection {
                start: f.up_start,
                end: f.up_end,
            },
        }
    }

    #[test]
    fn imported_bisket_definition_publishes_from_the_gltf_transform_host() {
        let mut world = World::default();
        let anchor = world.add_component(TransformComponent::new());
        let gltf = world.add_component(GLTFComponent::new("assets/models/bisket.glb"));
        world.add_child(anchor, gltf).unwrap();
        let source = world.add_component(JointRetargetBasisComponent::new(
            ComponentRef::Query("#J_Bip_L_Hand".into()),
            ComponentRef::Query("#J_Bip_L_Middle1".into()),
            ComponentRef::Query("#J_Bip_L_Middle3".into()),
            ComponentRef::Query("#J_Bip_L_Little1".into()),
            ComponentRef::Query("#J_Bip_L_Index1".into()),
        ));
        world.add_child(gltf, source).unwrap();

        let mut system = JointBasisRetargetingSystem::default();
        system.register_component(&world, source);
        assert_eq!(
            system.status_for_source(source),
            Some(RetargetBasisStatus::WaitingForGltf)
        );

        let mut gltf_system = GLTFSystem::new();
        gltf_system.register_component(gltf);
        let mut visuals = VisualWorld::default();
        let mut skinned_mesh = SkinnedMeshSystem::new();
        let mut renderable_system = RenderableSystem::default();
        let mut queue = CommandQueue::new();
        gltf_system.tick_with_queue(
            &mut world,
            &mut visuals,
            &mut skinned_mesh,
            &mut renderable_system,
            &mut queue,
            0.0,
        );
        system.gltf_initialized(&world, gltf);

        assert_eq!(
            system.status_for_source(source),
            Some(RetargetBasisStatus::Ready)
        );
        let hand = world
            .find_component(anchor, "#J_Bip_L_Hand")
            .expect("Bisket left hand joint");
        let basis = system.basis_for(hand).expect("ready Bisket hand basis");
        let forward = mat4_mul_vec4(
            basis.target_rest_to_canonical,
            [basis.forward[0], basis.forward[1], basis.forward[2], 0.0],
        );
        let up = mat4_mul_vec4(
            basis.target_rest_to_canonical,
            [basis.up[0], basis.up[1], basis.up[2], 0.0],
        );
        assert!(forward[0].abs() < 1e-4);
        assert!(forward[1].abs() < 1e-4);
        assert!((forward[2] + 1.0).abs() < 1e-4);
        assert!(up[0].abs() < 1e-4);
        assert!((up[1] - 1.0).abs() < 1e-4);
        assert!(up[2].abs() < 1e-4);
    }

    #[test]
    fn derives_finite_right_handed_canonical_axes_in_target_rest_space() {
        let mut f = fixture();
        let source = f.world.add_component(TransformComponent::new());
        let mut system = JointBasisRetargetingSystem::default();
        system.replace_definition(&f.world, source, definition(&f));
        let basis = system.basis_for(f.target).unwrap();
        assert_eq!(basis.forward, [0.0, 0.0, -1.0]);
        assert_eq!(basis.up, [1.0, 0.0, 0.0]);
        assert!(
            basis
                .canonical_to_target_rest
                .iter()
                .flatten()
                .all(|v| v.is_finite())
        );
        let canonical_forward =
            mat4_mul_vec4(basis.target_rest_to_canonical, [0.0, 0.0, -1.0, 0.0]);
        let canonical_up = mat4_mul_vec4(basis.target_rest_to_canonical, [1.0, 0.0, 0.0, 0.0]);
        assert!((canonical_forward[2] + 1.0).abs() < 1e-5);
        assert!((canonical_up[1] - 1.0).abs() < 1e-5);
        assert!(
            vec3_dot(
                vec3_cross(basis.right, basis.up),
                vec3_scale(basis.forward, -1.0)
            ) > 0.999
        );
    }

    #[test]
    fn duplicate_targets_conflict_and_removal_republishes_with_new_generation() {
        let mut f = fixture();
        let a = f.world.add_component(TransformComponent::new());
        let b = f.world.add_component(TransformComponent::new());
        let mut system = JointBasisRetargetingSystem::default();
        system.replace_definition(&f.world, a, definition(&f));
        let first_generation = system.basis_for(f.target).unwrap().generation;
        system.replace_definition(&f.world, b, definition(&f));
        assert!(system.basis_for(f.target).is_none());
        assert!(matches!(
            system.status_for(f.target),
            Some(RetargetBasisStatus::ConflictingDefinition { .. })
        ));
        system.remove_definition(&f.world, b);
        assert!(system.basis_for(f.target).unwrap().generation > first_generation);
        assert_eq!(
            system.status_for_source(a),
            Some(RetargetBasisStatus::Ready)
        );
    }

    #[test]
    fn invalid_replacement_atomically_removes_old_publication() {
        let mut f = fixture();
        let source = f.world.add_component(TransformComponent::new());
        let mut system = JointBasisRetargetingSystem::default();
        system.replace_definition(&f.world, source, definition(&f));
        let invalid = RetargetBasisDefinition {
            up: LandmarkDirection {
                start: f.forward_start,
                end: f.forward_end,
            },
            ..definition(&f)
        };
        system.replace_definition(&f.world, source, invalid);
        assert!(system.basis_for(f.target).is_none());
        assert!(matches!(
            system.status_for_source(source),
            Some(RetargetBasisStatus::Invalid(_))
        ));
    }

    #[test]
    fn authored_resolution_is_armature_scoped_and_waits_for_import() {
        let mut f = fixture();
        let declaration = f.world.add_component(JointRetargetBasisComponent::new(
            ComponentRef::Query("#hand".into()),
            ComponentRef::Query("#middle1".into()),
            ComponentRef::Query("#middle3".into()),
            ComponentRef::Query("#little1".into()),
            ComponentRef::Query("#index1".into()),
        ));
        f.world.add_child(f.gltf, declaration).unwrap();
        let joints = std::mem::take(
            &mut f
                .world
                .get_component_by_id_as_mut::<GLTFComponent>(f.gltf)
                .unwrap()
                .armature_joint_transforms,
        );
        let mut system = JointBasisRetargetingSystem::default();
        system.register_component(&f.world, declaration);
        assert_eq!(
            system.status_for_source(declaration),
            Some(RetargetBasisStatus::WaitingForGltf)
        );
        f.world
            .get_component_by_id_as_mut::<GLTFComponent>(f.gltf)
            .unwrap()
            .armature_joint_transforms = joints;
        system.gltf_initialized(&f.world, f.gltf);
        assert_eq!(
            system.status_for_source(declaration),
            Some(RetargetBasisStatus::Ready)
        );
    }

    #[test]
    fn rejects_zero_length_collinear_missing_and_singular_rest_geometry() {
        let mut f = fixture();
        let source = f.world.add_component(TransformComponent::new());
        let mut system = JointBasisRetargetingSystem::default();

        let zero = RetargetBasisDefinition {
            forward: LandmarkDirection {
                start: f.forward_start,
                end: f.forward_start,
            },
            ..definition(&f)
        };
        system.replace_definition(&f.world, source, zero);
        assert!(
            matches!(system.status_for_source(source), Some(RetargetBasisStatus::Invalid(message)) if message.contains("zero length"))
        );

        let collinear = RetargetBasisDefinition {
            up: LandmarkDirection {
                start: f.forward_start,
                end: f.forward_end,
            },
            ..definition(&f)
        };
        system.replace_definition(&f.world, source, collinear);
        assert!(
            matches!(system.status_for_source(source), Some(RetargetBasisStatus::Invalid(message)) if message.contains("collinear"))
        );

        let rest = f.world.children_of(f.up_end)[0];
        f.world.remove_component_subtree(rest).unwrap();
        system.replace_definition(&f.world, source, definition(&f));
        assert!(
            matches!(system.status_for_source(source), Some(RetargetBasisStatus::Invalid(message)) if message.contains("immutable rest pose"))
        );

        let singular_rest = f.world.add_component(BoneRestPoseComponent::new(
            [3.0, 3.0, 4.0],
            [0.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
        ));
        f.world.add_child(f.up_end, singular_rest).unwrap();
        let target_rest = f.world.children_of(f.target)[0];
        f.world.remove_component_subtree(target_rest).unwrap();
        let singular_target = f.world.add_component(BoneRestPoseComponent::new(
            [2.0, 3.0, 4.0],
            [0.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
        ));
        f.world.add_child(f.target, singular_target).unwrap();
        system.replace_definition(&f.world, source, definition(&f));
        assert!(
            matches!(system.status_for_source(source), Some(RetargetBasisStatus::Invalid(message)) if message.contains("singular"))
        );
    }
}
