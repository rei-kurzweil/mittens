use crate::engine::ecs::component::transform_operator::{
    TransformChannel, TransformSampleTranslationSettings,
};
use crate::engine::ecs::component::{ComponentRef, QueryRootMode, parse_scoped_query};
use crate::engine::ecs::component::{
    TransformCameraSpecificMode, TransformComponent, TransformForkTrsSettings,
    TransformOperatorComponent, TransformOperatorKind, TransformQuatOp, TransformVec3Op,
};
use crate::engine::ecs::system::System;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::VisualWorld;
use crate::engine::graphics::primitives::TransformMatrix;
use crate::engine::transform::TransformTrs;
use crate::engine::user_input::InputState;
use crate::utils::math;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformOperatorInput {
    ParentWorld,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransformOperatorStage {
    ForkTrs(TransformForkTrsSettings),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransformOperatorPlan {
    pub owner_component: Option<ComponentId>,
    pub input: TransformOperatorInput,
    pub stages: Vec<TransformOperatorStage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TransformOperatorStageKey {
    channel: TransformChannel,
    owner_component: Option<ComponentId>,
    stage_path: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
struct QuatTemporalState {
    output_quat_xyzw: [f32; 4],
    last_input_quat_xyzw: [f32; 4],
    debug_window: QuatTemporalDebugWindow,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct QuatTemporalDebugWindow {
    raw_step_deg: VecDeque<f32>,
    filtered_step_deg: VecDeque<f32>,
    lag_deg: VecDeque<f32>,
    sample_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Vec3TemporalState {
    output_vec3: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslationSampleStatus {
    Pending,
    Ready,
    MissingSource,
    AmbiguousSource,
    InvalidTarget,
    DependencyCycle,
    NonFinite,
}

#[derive(Debug, Clone)]
struct TranslationSampleState {
    authored: TransformSampleTranslationSettings,
    revision: u64,
    relation_revision: u64,
    source: Option<ComponentId>,
    status: TranslationSampleStatus,
    input: TransformMatrix,
}

#[derive(Debug, Default)]
pub struct TransformOperatorSystem {
    last_dt_sec: Option<f32>,
    translation_samples: HashMap<ComponentId, TranslationSampleState>,
    translation_dependents: HashMap<ComponentId, HashSet<ComponentId>>,
    sample_relation_revision: u64,
    sample_refresh_revision: u64,
    sample_refresh_relation_revision: u64,
    reference_routes: HashMap<ComponentId, TransformOperatorKind>,
    changed_translation_sources: HashSet<ComponentId>,
    vec3_temporal_state: HashMap<TransformOperatorStageKey, Vec3TemporalState>,
    quat_temporal_state: HashMap<TransformOperatorStageKey, QuatTemporalState>,
    yaw_follow_state: HashMap<TransformOperatorStageKey, f32>,
    stereoscopic_active: bool,
    /// Pre-camera and effective matrices for camera-specific anchors. A direct refresh of an
    /// anchor starts from its cached `matrix_world`, which is the previous effective matrix;
    /// retaining the unmodified basis prevents the selected settings transform compounding.
    camera_specific_basis: HashMap<ComponentId, (TransformMatrix, TransformMatrix)>,
    /// Last valid (input, inverse-local) pair for referenced inverse-local
    /// operators. Retaining the inverse lets the inherited pose continue to
    /// update without snapping to identity while a source is temporarily
    /// unresolved or singular.
    inverse_local_state: HashMap<ComponentId, (TransformMatrix, TransformMatrix)>,
    /// Reverse dependency index populated when inverse-local operators resolve.
    /// Transform invalidation can therefore visit only consumers of the changed
    /// source instead of scanning the world.
    inverse_local_source_by_operator: HashMap<ComponentId, ComponentId>,
    inverse_local_dependents_by_source: HashMap<ComponentId, HashSet<ComponentId>>,
}

impl TransformOperatorSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_operator_boundary(&self, world: &World, cid: ComponentId) -> bool {
        world
            .get_component_by_id_as::<TransformOperatorComponent>(cid)
            .is_some_and(|operator| {
                !matches!(operator.kind, TransformOperatorKind::CameraSpecific(_))
            })
            || Self::camera_specific_settings(world, cid).is_some()
    }

    pub fn set_stereoscopic_active(&mut self, active: bool) {
        self.stereoscopic_active = active;
    }

    fn camera_specific_settings(
        world: &World,
        anchor: ComponentId,
    ) -> Option<(Option<ComponentId>, Option<ComponentId>)> {
        let mut mono = None;
        let mut stereo = None;
        for &child in world.children_of(anchor) {
            let Some(c) = world
                .get_component_by_id_as::<TransformOperatorComponent>(child)
                .and_then(|operator| operator.camera_settings())
            else {
                continue;
            };
            let settings = world.children_of(child).iter().copied().find(|&id| {
                world
                    .get_component_by_id_as::<TransformComponent>(id)
                    .is_some()
            });
            match c.mode {
                TransformCameraSpecificMode::Monoscopic => mono = settings,
                TransformCameraSpecificMode::Stereoscopic => stereo = settings,
            }
        }
        (mono.is_some() || stereo.is_some()).then_some((mono, stereo))
    }

    /// Snapshot a fork's configuration for standalone evaluation. Hierarchy
    /// propagation evaluates the borrowed live settings directly.
    pub fn plan_for_operator(
        &self,
        world: &World,
        root: ComponentId,
    ) -> Option<TransformOperatorPlan> {
        let settings = world
            .get_component_by_id_as::<TransformOperatorComponent>(root)?
            .fork_trs_settings()?;
        Some(TransformOperatorPlan {
            owner_component: Some(root),
            input: TransformOperatorInput::ParentWorld,
            stages: vec![TransformOperatorStage::ForkTrs(settings.clone())],
        })
    }

    pub fn evaluate_operator(
        &mut self,
        world: &World,
        root: ComponentId,
        input_world: TransformMatrix,
    ) -> Option<(TransformMatrix, Vec<ComponentId>)> {
        // Source configuration edits on other routing kinds can create/remove
        // sample feedback without changing structural topology. Observe their
        // authored values once per change, outside the steady sample hot path.
        let route = world
            .get_component_by_id_as::<TransformOperatorComponent>(root)
            .map(|operator| &operator.kind)
            .filter(|kind| {
                matches!(
                    kind,
                    TransformOperatorKind::Parent(_) | TransformOperatorKind::InverseLocal(_)
                )
            });
        if let Some(kind) = route {
            if self.reference_routes.get(&root) != Some(kind) {
                self.reference_routes.insert(root, kind.clone());
                self.sample_relation_revision = self.sample_relation_revision.wrapping_add(1);
            }
        } else if self.reference_routes.remove(&root).is_some() {
            self.sample_relation_revision = self.sample_relation_revision.wrapping_add(1);
        }
        let rebased_world = Self::apply_transform_parent_basis(world, root, input_world);
        if let Some(settings) = world
            .get_component_by_id_as::<TransformOperatorComponent>(root)
            .and_then(|operator| operator.sample_translation_settings())
        {
            let output = self.evaluate_translation_sample(world, root, settings, rebased_world);
            return Some((output, world.children_of(root).to_vec()));
        }

        if let Some(operator) = world
            .get_component_by_id_as::<TransformOperatorComponent>(root)
            .and_then(|operator| operator.inverse_local_settings())
        {
            let source = operator.resolve_source_component(world, root);
            if let Some(source) = source {
                self.register_inverse_local_dependency(root, source);
            }
            let source_is_valid = source.is_some_and(|source| {
                world
                    .get_component_by_id_as::<TransformComponent>(source)
                    .is_some()
                    && !Self::is_descendant_of(world, source, root)
            });
            if source_is_valid {
                let source_local = world
                    .get_component_by_id_as::<TransformComponent>(source.unwrap())
                    .expect("validated transform source")
                    .transform
                    .model;
                if let Some(inverse) = math::mat4_inverse(source_local) {
                    self.inverse_local_state
                        .insert(root, (rebased_world, inverse));
                    let effective = math::mat4_mul(rebased_world, inverse);
                    return Some((effective, world.children_of(root).to_vec()));
                }
            }

            // A previously valid relationship keeps its last result. On the
            // first invalid evaluation, suppress downstream propagation rather
            // than silently treating the missing inverse as identity.
            return Some(match self.inverse_local_state.get(&root).copied() {
                Some((_input, inverse)) => (
                    math::mat4_mul(rebased_world, inverse),
                    world.children_of(root).to_vec(),
                ),
                None => (rebased_world, Vec::new()),
            });
        }

        if let Some((mono, stereo)) = Self::camera_specific_settings(world, root) {
            let basis = match self.camera_specific_basis.get(&root).copied() {
                Some((basis, previous_effective)) if input_world == previous_effective => basis,
                _ => rebased_world,
            };
            let selected = if self.stereoscopic_active {
                stereo
            } else {
                mono
            };
            let effective = selected
                .and_then(|id| world.get_component_by_id_as::<TransformComponent>(id))
                .map(|t| math::mat4_mul(basis, t.transform.model))
                .unwrap_or(basis);
            self.camera_specific_basis.insert(root, (basis, effective));
            let outputs = world
                .children_of(root)
                .iter()
                .copied()
                .filter(|&id| {
                    world
                        .get_component_by_id_as::<TransformOperatorComponent>(id)
                        .and_then(|operator| operator.camera_settings())
                        .is_none()
                })
                .collect();
            return Some((effective, outputs));
        }

        if let Some(settings) = world
            .get_component_by_id_as::<TransformOperatorComponent>(root)
            .and_then(|operator| operator.fork_trs_settings())
        {
            let channels = Self::decompose_matrix(rebased_world);
            let output = self.evaluate_fork_trs(
                Some(root),
                settings,
                channels,
                &[0],
                world,
                self.last_dt_sec,
            );
            return Some((
                Self::recompose_matrix(output),
                world.children_of(root).to_vec(),
            ));
        }

        if world
            .get_component_by_id_as::<TransformOperatorComponent>(root)
            .and_then(|operator| operator.parent_settings())
            .is_some()
        {
            return Some((rebased_world, world.children_of(root).to_vec()));
        }

        None
    }

    fn unique_ref(
        world: &World,
        owner: ComponentId,
        source: &ComponentRef,
        scoped_root: Option<ComponentId>,
    ) -> Result<ComponentId, TranslationSampleStatus> {
        match source {
            ComponentRef::Guid(guid) => world
                .component_id_by_guid(*guid)
                .ok_or(TranslationSampleStatus::MissingSource),
            ComponentRef::Query(query) => {
                let scoped = parse_scoped_query(query);
                let mode = if query == scoped.selector {
                    if scoped_root.is_some() {
                        QueryRootMode::SelfSubtree
                    } else {
                        QueryRootMode::WorldRoot
                    }
                } else {
                    scoped.root_mode
                };
                let selector = scoped.selector.trim();
                if selector.is_empty() {
                    return Err(TranslationSampleStatus::MissingSource);
                }
                let matches = match mode {
                    QueryRootMode::WorldRoot if selector.starts_with('>') => world
                        .world_roots()
                        .into_iter()
                        .filter(|root| {
                            world.component_matches_selector(*root, selector[1..].trim())
                        })
                        .collect::<Vec<_>>(),
                    QueryRootMode::WorldRoot => world
                        .world_roots()
                        .into_iter()
                        .flat_map(|root| world.find_all_components(root, selector))
                        .collect::<Vec<_>>(),
                    QueryRootMode::SelfSubtree => {
                        world.find_all_components(scoped_root.unwrap_or(owner), selector)
                    }
                    QueryRootMode::ParentScope { levels_up } => {
                        let mut root = scoped_root.unwrap_or(owner);
                        for _ in 0..levels_up {
                            root = world
                                .parent_of(root)
                                .ok_or(TranslationSampleStatus::MissingSource)?;
                        }
                        world.find_all_components(root, selector)
                    }
                };
                match matches.as_slice() {
                    [id] => Ok(*id),
                    [] => Err(TranslationSampleStatus::MissingSource),
                    _ => Err(TranslationSampleStatus::AmbiguousSource),
                }
            }
        }
    }

    fn resolve_translation_source(
        world: &World,
        owner: ComponentId,
        settings: &TransformSampleTranslationSettings,
    ) -> Result<ComponentId, TranslationSampleStatus> {
        let source = settings
            .source
            .as_ref()
            .ok_or(TranslationSampleStatus::MissingSource)?;
        let root = if matches!(source, ComponentRef::Query(_)) {
            settings
                .root_source
                .as_ref()
                .map(|root| Self::unique_ref(world, owner, root, None))
                .transpose()?
        } else {
            None
        };
        let id = Self::unique_ref(world, owner, source, root)?;
        if world
            .get_component_by_id_as::<TransformComponent>(id)
            .is_none()
        {
            return Err(TranslationSampleStatus::InvalidTarget);
        }
        Ok(id)
    }

    fn sample_depends_on(
        &self,
        world: &World,
        owner: ComponentId,
        source: ComponentId,
        depth: usize,
    ) -> bool {
        if depth > 64 {
            return true;
        }
        let mut node = Some(source);
        while let Some(id) = node {
            if id == owner {
                return true;
            }
            if let Some(dependency) = self
                .translation_samples
                .get(&id)
                .and_then(|state| state.source)
            {
                if self.sample_depends_on(world, owner, dependency, depth + 1) {
                    return true;
                }
            }
            if let Some(operator) = world.get_component_by_id_as::<TransformOperatorComponent>(id) {
                let dependency = operator
                    .parent_settings()
                    .and_then(|settings| settings.resolve_target_component(world))
                    .or_else(|| {
                        operator
                            .inverse_local_settings()
                            .and_then(|settings| settings.resolve_source_component(world, id))
                    });
                if let Some(dependency) = dependency {
                    if self.sample_depends_on(world, owner, dependency, depth + 1) {
                        return true;
                    }
                }
            }
            node = world.parent_of(id);
        }
        false
    }

    fn evaluate_translation_sample(
        &mut self,
        world: &World,
        owner: ComponentId,
        settings: &TransformSampleTranslationSettings,
        input: TransformMatrix,
    ) -> TransformMatrix {
        let revision = world.query_revision();
        let needs_resolution = self.translation_samples.get(&owner).is_none_or(|state| {
            state.authored != *settings
                || state.revision != revision
                || state.relation_revision != self.sample_relation_revision
        });
        if needs_resolution {
            let resolved = Self::resolve_translation_source(world, owner, settings);
            let previous = self
                .translation_samples
                .get(&owner)
                .and_then(|state| state.source);
            let source = resolved.as_ref().ok().copied();
            if previous != source {
                if let Some(previous) = previous {
                    self.remove_translation_dependent(previous, owner);
                }
                if let Some(source) = source {
                    self.translation_dependents
                        .entry(source)
                        .or_default()
                        .insert(owner);
                }
                self.sample_relation_revision = self.sample_relation_revision.wrapping_add(1);
            }
            let status = match resolved {
                Ok(source) if self.sample_depends_on(world, owner, source, 0) => {
                    TranslationSampleStatus::DependencyCycle
                }
                Ok(_) => TranslationSampleStatus::Ready,
                Err(status) => status,
            };
            self.translation_samples.insert(
                owner,
                TranslationSampleState {
                    authored: settings.clone(),
                    revision,
                    relation_revision: self.sample_relation_revision,
                    source,
                    status,
                    input,
                },
            );
        }
        let state = self.translation_samples.get_mut(&owner).unwrap();
        state.input = input;
        // Non-finite sources can recover after an ordinary typed transform write.
        if !matches!(
            state.status,
            TranslationSampleStatus::Ready | TranslationSampleStatus::NonFinite
        ) {
            return input;
        }
        let Some(source) = state
            .source
            .and_then(|id| world.get_component_by_id_as::<TransformComponent>(id))
        else {
            state.status = TranslationSampleStatus::MissingSource;
            return input;
        };
        let mut output = input;
        for axis in 0..3 {
            if settings.axes.contains(axis) {
                let value = source.transform.matrix_world[3][axis];
                if !value.is_finite() {
                    state.status = TranslationSampleStatus::NonFinite;
                    return input;
                }
                output[3][axis] = value;
            }
        }
        state.status = TranslationSampleStatus::Ready;
        output
    }

    pub fn translation_sample_status(
        &self,
        world: &World,
        owner: ComponentId,
    ) -> TranslationSampleStatus {
        let Some(settings) = world
            .get_component_by_id_as::<TransformOperatorComponent>(owner)
            .and_then(|operator| operator.sample_translation_settings())
        else {
            return TranslationSampleStatus::InvalidTarget;
        };
        let Some(state) = self.translation_samples.get(&owner) else {
            return TranslationSampleStatus::Pending;
        };
        if state.authored != *settings
            || state.revision != world.query_revision()
            || state.relation_revision != self.sample_relation_revision
        {
            return TranslationSampleStatus::Pending;
        }
        if state.source.is_some_and(|source| {
            world
                .get_component_by_id_as::<TransformComponent>(source)
                .is_none()
        }) {
            return TranslationSampleStatus::MissingSource;
        }
        state.status
    }

    pub fn translation_sample_input(&self, owner: ComponentId) -> Option<TransformMatrix> {
        self.translation_samples
            .get(&owner)
            .map(|state| state.input)
    }

    pub fn mark_translation_source_changed(&mut self, source: ComponentId) {
        if self.translation_dependents.contains_key(&source) {
            self.changed_translation_sources.insert(source);
        }
    }

    pub fn translation_sample_input_is_current(&self, world: &World, owner: ComponentId) -> bool {
        self.translation_samples
            .get(&owner)
            .is_some_and(|state| state.revision == world.query_revision())
    }
    pub fn has_changed_translation_sources(&self) -> bool {
        !self.changed_translation_sources.is_empty()
            || self.sample_refresh_relation_revision != self.sample_relation_revision
    }

    /// Refresh only retained consumers. Topology/selector edits invalidate the
    /// registered sample set; ordinary pose changes visit the indexed sources.
    pub fn translation_sample_dependents(
        &mut self,
        world: &World,
        changed: Option<ComponentId>,
    ) -> Vec<ComponentId> {
        if self.sample_refresh_revision != world.query_revision()
            || self.sample_refresh_relation_revision != self.sample_relation_revision
        {
            self.sample_refresh_revision = world.query_revision();
            self.sample_refresh_relation_revision = self.sample_relation_revision;
            self.changed_translation_sources.clear();
            return self
                .translation_samples
                .keys()
                .copied()
                .filter(|id| world.get_component_record(*id).is_some())
                .collect();
        }
        let mut out = Vec::new();
        if let Some(owners) = changed.and_then(|source| self.translation_dependents.get(&source)) {
            out.extend(owners.iter().copied());
        }
        for source in self.changed_translation_sources.drain() {
            if let Some(owners) = self.translation_dependents.get(&source) {
                out.extend(owners.iter().copied());
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    fn remove_translation_dependent(&mut self, source: ComponentId, owner: ComponentId) {
        if let Some(owners) = self.translation_dependents.get_mut(&source) {
            owners.remove(&owner);
            if owners.is_empty() {
                self.translation_dependents.remove(&source);
                self.changed_translation_sources.remove(&source);
            }
        }
    }

    pub fn component_removed(&mut self, id: ComponentId) {
        self.reference_routes.remove(&id);
        if let Some(state) = self.translation_samples.remove(&id) {
            if let Some(source) = state.source {
                self.remove_translation_dependent(source, id);
            }
        }
        if let Some(owners) = self.translation_dependents.remove(&id) {
            for owner in owners {
                if let Some(state) = self.translation_samples.get_mut(&owner) {
                    state.status = TranslationSampleStatus::MissingSource;
                }
            }
        }
        self.sample_relation_revision = self.sample_relation_revision.wrapping_add(1);
    }

    pub fn inverse_local_cached_input(&self, component: ComponentId) -> Option<TransformMatrix> {
        self.inverse_local_state
            .get(&component)
            .map(|(input, _inverse)| *input)
    }

    pub fn inverse_local_dependents(&self, source: ComponentId) -> Vec<ComponentId> {
        self.inverse_local_dependents_by_source
            .get(&source)
            .map(|dependents| dependents.iter().copied().collect())
            .unwrap_or_default()
    }

    fn register_inverse_local_dependency(&mut self, operator: ComponentId, source: ComponentId) {
        if self.inverse_local_source_by_operator.get(&operator) == Some(&source) {
            return;
        }

        if let Some(previous_source) = self
            .inverse_local_source_by_operator
            .insert(operator, source)
            && let Some(previous_dependents) = self
                .inverse_local_dependents_by_source
                .get_mut(&previous_source)
        {
            previous_dependents.remove(&operator);
            if previous_dependents.is_empty() {
                self.inverse_local_dependents_by_source
                    .remove(&previous_source);
            }
        }
        self.inverse_local_dependents_by_source
            .entry(source)
            .or_default()
            .insert(operator);
    }

    fn is_descendant_of(world: &World, mut node: ComponentId, ancestor: ComponentId) -> bool {
        while let Some(parent) = world.parent_of(node) {
            if parent == ancestor {
                return true;
            }
            node = parent;
        }
        false
    }

    fn apply_transform_parent_basis(
        world: &World,
        node: ComponentId,
        current_world: TransformMatrix,
    ) -> TransformMatrix {
        world
            .get_component_by_id_as::<TransformOperatorComponent>(node)
            .and_then(|operator| operator.parent_settings())
            .and_then(|tp| tp.resolve_target_component(world))
            .and_then(|target| Self::world_model(world, target))
            .unwrap_or(current_world)
    }

    fn world_model(world: &World, cid: ComponentId) -> Option<TransformMatrix> {
        if let Some(t) = world.get_component_by_id_as::<TransformComponent>(cid) {
            return Some(t.transform.matrix_world);
        }

        let mut cur = cid;
        while let Some(parent) = world.parent_of(cur) {
            if let Some(t) = world.get_component_by_id_as::<TransformComponent>(parent) {
                return Some(t.transform.matrix_world);
            }
            cur = parent;
        }
        None
    }

    pub fn controller_rotation_smoothing_plan(
        owner_component: Option<ComponentId>,
        smoothing_factor: f32,
    ) -> TransformOperatorPlan {
        TransformOperatorPlan {
            owner_component,
            input: TransformOperatorInput::ParentWorld,
            stages: vec![TransformOperatorStage::ForkTrs(TransformForkTrsSettings {
                translation_ops: vec![TransformVec3Op::Pass],
                rotation_ops: vec![TransformQuatOp::TemporalFilter { smoothing_factor }],
                scale_ops: vec![TransformVec3Op::Pass],
            })],
        }
    }

    pub fn evaluate_plan(
        &mut self,
        operator: &TransformOperatorPlan,
        input_world: TransformMatrix,
        world: &World,
        dt_sec: Option<f32>,
    ) -> TransformMatrix {
        let mut channels = Self::decompose_matrix(input_world);
        for (stage_index, stage) in operator.stages.iter().enumerate() {
            let mut stage_path = vec![stage_index];
            channels = self.evaluate_stage(
                operator.owner_component,
                stage,
                channels,
                &mut stage_path,
                world,
                dt_sec,
            );
        }
        Self::recompose_matrix(channels)
    }

    fn evaluate_stage(
        &mut self,
        owner_component: Option<ComponentId>,
        stage: &TransformOperatorStage,
        input: TransformTrs,
        stage_path: &mut Vec<usize>,
        world: &World,
        dt_sec: Option<f32>,
    ) -> TransformTrs {
        match stage {
            TransformOperatorStage::ForkTrs(fork) => {
                self.evaluate_fork_trs(owner_component, fork, input, stage_path, world, dt_sec)
            }
        }
    }

    fn evaluate_fork_trs(
        &mut self,
        owner_component: Option<ComponentId>,
        fork: &TransformForkTrsSettings,
        input: TransformTrs,
        stage_path: &[usize],
        world: &World,
        dt_sec: Option<f32>,
    ) -> TransformTrs {
        let translation = self.apply_vec3_ops(
            owner_component,
            &fork.translation_ops,
            TransformChannel::Translation,
            input.translation,
            [0.0, 0.0, 0.0],
            stage_path,
            world,
            dt_sec,
        );
        let rotation_quat_xyzw = self.apply_quat_ops(
            owner_component,
            &fork.rotation_ops,
            input.rotation_quat_xyzw,
            [0.0, 0.0, 0.0, 1.0],
            stage_path,
            world,
            dt_sec,
        );
        let scale = self.apply_vec3_ops(
            owner_component,
            &fork.scale_ops,
            TransformChannel::Scale,
            input.scale,
            [1.0, 1.0, 1.0],
            stage_path,
            world,
            dt_sec,
        );

        TransformTrs {
            translation,
            rotation_quat_xyzw,
            scale,
        }
    }

    /// Walk ancestor TransformComponents from `owner` upward. Returns the world matrix of the
    /// `skip`-th TransformComponent found (0 = first / nearest ancestor, 1 = second, etc.).
    fn sample_ancestor_world(
        world: &World,
        owner: ComponentId,
        skip: usize,
    ) -> Option<TransformMatrix> {
        let mut found = 0usize;
        let mut cur = owner;
        while let Some(parent) = world.parent_of(cur) {
            if let Some(t) = world.get_component_by_id_as::<TransformComponent>(parent) {
                if found == skip {
                    return Some(t.transform.matrix_world);
                }
                found += 1;
            }
            cur = parent;
        }
        None
    }

    fn apply_vec3_ops(
        &mut self,
        owner_component: Option<ComponentId>,
        ops: &[TransformVec3Op],
        channel: TransformChannel,
        input: [f32; 3],
        dropped_value: [f32; 3],
        stage_path: &[usize],
        world: &World,
        dt_sec: Option<f32>,
    ) -> [f32; 3] {
        let mut current = input;
        for (op_index, op) in ops.iter().enumerate() {
            current = match *op {
                TransformVec3Op::Pass => current,
                TransformVec3Op::Drop => dropped_value,
                TransformVec3Op::SampleAncestorTranslation { skip } => owner_component
                    .and_then(|owner| Self::sample_ancestor_world(world, owner, skip))
                    .map(|m| [m[3][0], m[3][1], m[3][2]])
                    .unwrap_or(current),
                TransformVec3Op::TemporalSmooth { smoothing_factor } => {
                    let mut full_path = stage_path.to_vec();
                    full_path.push(op_index);
                    let key = TransformOperatorStageKey {
                        channel,
                        owner_component,
                        stage_path: full_path,
                    };
                    let alpha = Self::alpha_from_smoothing_factor(smoothing_factor, dt_sec);
                    let previous = self
                        .vec3_temporal_state
                        .get(&key)
                        .map(|state| state.output_vec3)
                        .unwrap_or(current);
                    let filtered = Self::vec3_lerp(previous, current, alpha);
                    self.vec3_temporal_state.insert(
                        key,
                        Vec3TemporalState {
                            output_vec3: filtered,
                        },
                    );
                    filtered
                }
            };
        }
        current
    }

    fn apply_quat_ops(
        &mut self,
        owner_component: Option<ComponentId>,
        ops: &[TransformQuatOp],
        input: [f32; 4],
        dropped_value: [f32; 4],
        stage_path: &[usize],
        world: &World,
        dt_sec: Option<f32>,
    ) -> [f32; 4] {
        let mut current = Self::quat_normalize(input);
        for (op_index, op) in ops.iter().enumerate() {
            current = match *op {
                TransformQuatOp::Pass => current,
                TransformQuatOp::Drop => dropped_value,
                TransformQuatOp::SampleAncestorRotation { skip } => owner_component
                    .and_then(|owner| Self::sample_ancestor_world(world, owner, skip))
                    .map(|m| Self::decompose_matrix(m).rotation_quat_xyzw)
                    .unwrap_or(current),
                TransformQuatOp::ExtractYaw => {
                    // Project onto Y-rotation subspace: normalize([0, q.y, 0, q.w])
                    let (qy, qw) = (current[1], current[3]);
                    let len = (qy * qy + qw * qw).sqrt().max(1e-8);
                    [0.0, qy / len, 0.0, qw / len]
                }
                TransformQuatOp::YawFollow {
                    threshold,
                    rate,
                    initial_yaw,
                    forward_plus_z,
                } => {
                    let mut full_path = stage_path.to_vec();
                    full_path.push(op_index);
                    let key = TransformOperatorStageKey {
                        channel: TransformChannel::Rotation,
                        owner_component,
                        stage_path: full_path,
                    };
                    let head_yaw = Self::extract_yaw_from_quat(current, forward_plus_z);
                    let body_yaw = self
                        .yaw_follow_state
                        .get(&key)
                        .copied()
                        .unwrap_or(initial_yaw);
                    let new_body_yaw = if let Some(dt) = dt_sec {
                        let delta = Self::signed_yaw_diff(head_yaw, body_yaw);
                        if delta.abs() > threshold {
                            let target = head_yaw - delta.signum() * threshold;
                            let step = rate * dt;
                            Self::lerp_angle(
                                body_yaw,
                                target,
                                step.min(delta.abs()) / delta.abs().max(1e-9),
                            )
                        } else {
                            body_yaw
                        }
                    } else {
                        body_yaw
                    };
                    self.yaw_follow_state.insert(key, new_body_yaw);
                    Self::quat_rotation_y(new_body_yaw)
                }
                TransformQuatOp::TemporalFilter { smoothing_factor } => {
                    let mut full_path = stage_path.to_vec();
                    full_path.push(op_index);
                    let key = TransformOperatorStageKey {
                        channel: TransformChannel::Rotation,
                        owner_component,
                        stage_path: full_path.clone(),
                    };
                    let alpha = Self::alpha_from_smoothing_factor(smoothing_factor, dt_sec);
                    let previous_state = self.quat_temporal_state.get(&key).cloned();
                    let previous_output = previous_state
                        .as_ref()
                        .map(|state| state.output_quat_xyzw)
                        .unwrap_or(current);
                    let previous_input = previous_state
                        .as_ref()
                        .map(|state| state.last_input_quat_xyzw)
                        .unwrap_or(current);
                    let filtered = previous_state
                        .as_ref()
                        .map(|state| state.output_quat_xyzw)
                        .unwrap_or(current);

                    let mut next_state = previous_state.unwrap_or(QuatTemporalState {
                        output_quat_xyzw: filtered,
                        last_input_quat_xyzw: current,
                        debug_window: QuatTemporalDebugWindow::default(),
                    });
                    next_state.output_quat_xyzw = filtered;
                    next_state.last_input_quat_xyzw = current;

                    if Self::debug_quat_filter_enabled() {
                        let window_len = Self::debug_quat_filter_window_len();
                        let raw_step_deg = Self::quat_angle_degrees(previous_input, current);
                        let filtered_step_deg = Self::quat_angle_degrees(previous_output, filtered);
                        let lag_deg = Self::quat_angle_degrees(filtered, current);

                        Self::push_rolling_sample(
                            &mut next_state.debug_window.raw_step_deg,
                            raw_step_deg,
                            window_len,
                        );
                        Self::push_rolling_sample(
                            &mut next_state.debug_window.filtered_step_deg,
                            filtered_step_deg,
                            window_len,
                        );
                        Self::push_rolling_sample(
                            &mut next_state.debug_window.lag_deg,
                            lag_deg,
                            window_len,
                        );
                        next_state.debug_window.sample_count += 1;

                        if next_state.debug_window.sample_count % window_len as u64 == 0 {
                            let avg_raw = Self::rolling_avg(&next_state.debug_window.raw_step_deg);
                            let avg_filtered =
                                Self::rolling_avg(&next_state.debug_window.filtered_step_deg);
                            let avg_lag = Self::rolling_avg(&next_state.debug_window.lag_deg);
                            let max_raw = Self::rolling_max(&next_state.debug_window.raw_step_deg);
                            let max_filtered =
                                Self::rolling_max(&next_state.debug_window.filtered_step_deg);
                            let attenuation_pct = if avg_raw > 1e-4 {
                                (1.0 - (avg_filtered / avg_raw)).clamp(-10.0, 1.0) * 100.0
                            } else {
                                0.0
                            };

                            eprintln!(
                                "[TransformOperator][QuatFilter] owner={owner_component:?} stage_path={full_path:?} smoothing_factor={smoothing_factor:.3} dt={:.5} alpha={alpha:.5} raw_avg_deg={avg_raw:.3} filtered_avg_deg={avg_filtered:.3} lag_avg_deg={avg_lag:.3} raw_max_deg={max_raw:.3} filtered_max_deg={max_filtered:.3} attenuation_pct={attenuation_pct:.1} window={} samples={}",
                                dt_sec.unwrap_or(0.0),
                                next_state.debug_window.raw_step_deg.len(),
                                next_state.debug_window.sample_count,
                            );
                        }
                    }

                    self.quat_temporal_state.insert(key, next_state);
                    filtered
                }
            };
        }
        Self::quat_normalize(current)
    }

    fn extract_yaw_from_quat(q: [f32; 4], forward_plus_z: bool) -> f32 {
        let z = math::quat_rotate_vec3(q, [0.0, 0.0, 1.0]);
        if forward_plus_z {
            z[0].atan2(z[2])
        } else {
            (-z[0]).atan2(-z[2])
        }
    }

    fn quat_rotation_y(yaw: f32) -> [f32; 4] {
        let h = yaw * 0.5;
        [0.0, h.sin(), 0.0, h.cos()]
    }

    fn signed_yaw_diff(a: f32, b: f32) -> f32 {
        let pi = std::f32::consts::PI;
        let mut d = (a - b) % (2.0 * pi);
        if d > pi {
            d -= 2.0 * pi;
        }
        if d < -pi {
            d += 2.0 * pi;
        }
        d
    }

    fn lerp_angle(from: f32, to: f32, t: f32) -> f32 {
        from + Self::signed_yaw_diff(to, from) * t.clamp(0.0, 1.0)
    }

    fn decompose_matrix(m: TransformMatrix) -> TransformTrs {
        fn col3(m: TransformMatrix, c: usize) -> [f32; 3] {
            [m[c][0], m[c][1], m[c][2]]
        }

        let translation = [m[3][0], m[3][1], m[3][2]];
        let b0 = col3(m, 0);
        let b1 = col3(m, 1);
        let b2 = col3(m, 2);

        let scale = [
            math::vec3_len(b0).max(1e-8),
            math::vec3_len(b1).max(1e-8),
            math::vec3_len(b2).max(1e-8),
        ];

        let [x, y, z] = Self::orthonormalize_basis(b0, b1, b2);
        let rotation_quat_xyzw = Self::quat_from_basis_columns(x, y, z);

        TransformTrs {
            translation,
            rotation_quat_xyzw,
            scale,
        }
    }

    fn recompose_matrix(channels: TransformTrs) -> TransformMatrix {
        let x = math::quat_rotate_vec3(channels.rotation_quat_xyzw, [1.0, 0.0, 0.0]);
        let y = math::quat_rotate_vec3(channels.rotation_quat_xyzw, [0.0, 1.0, 0.0]);
        let z = math::quat_rotate_vec3(channels.rotation_quat_xyzw, [0.0, 0.0, 1.0]);

        let mut out = math::mat4_identity();
        out[0][0] = x[0] * channels.scale[0];
        out[0][1] = x[1] * channels.scale[0];
        out[0][2] = x[2] * channels.scale[0];

        out[1][0] = y[0] * channels.scale[1];
        out[1][1] = y[1] * channels.scale[1];
        out[1][2] = y[2] * channels.scale[1];

        out[2][0] = z[0] * channels.scale[2];
        out[2][1] = z[1] * channels.scale[2];
        out[2][2] = z[2] * channels.scale[2];

        out[3][0] = channels.translation[0];
        out[3][1] = channels.translation[1];
        out[3][2] = channels.translation[2];
        out
    }

    fn orthonormalize_basis(b0: [f32; 3], b1: [f32; 3], b2: [f32; 3]) -> [[f32; 3]; 3] {
        let x = math::vec3_normalize(b0);
        let y_proj = math::vec3_sub(b1, math::vec3_scale(x, math::vec3_dot(b1, x)));
        let mut y = math::vec3_normalize(y_proj);
        let mut z = math::vec3_cross(x, y);
        if math::vec3_len(z) < 1e-6 {
            z = math::vec3_normalize(b2);
            y = math::vec3_normalize(math::vec3_cross(z, x));
        } else {
            z = math::vec3_normalize(z);
            y = math::vec3_normalize(math::vec3_cross(z, x));
        }
        [x, y, z]
    }

    fn quat_from_basis_columns(x: [f32; 3], y: [f32; 3], z: [f32; 3]) -> [f32; 4] {
        let r00 = x[0];
        let r01 = y[0];
        let r02 = z[0];
        let r10 = x[1];
        let r11 = y[1];
        let r12 = z[1];
        let r20 = x[2];
        let r21 = y[2];
        let r22 = z[2];

        let trace = r00 + r11 + r22;
        let (qx, qy, qz, qw) = if trace > 0.0 {
            let s = (trace + 1.0).sqrt() * 2.0;
            ((r21 - r12) / s, (r02 - r20) / s, (r10 - r01) / s, 0.25 * s)
        } else if r00 > r11 && r00 > r22 {
            let s = (1.0 + r00 - r11 - r22).sqrt() * 2.0;
            (0.25 * s, (r01 + r10) / s, (r02 + r20) / s, (r21 - r12) / s)
        } else if r11 > r22 {
            let s = (1.0 + r11 - r00 - r22).sqrt() * 2.0;
            ((r01 + r10) / s, 0.25 * s, (r12 + r21) / s, (r02 - r20) / s)
        } else {
            let s = (1.0 + r22 - r00 - r11).sqrt() * 2.0;
            ((r02 + r20) / s, (r12 + r21) / s, 0.25 * s, (r10 - r01) / s)
        };

        Self::quat_normalize([qx, qy, qz, qw])
    }

    fn quat_normalize(q: [f32; 4]) -> [f32; 4] {
        let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
        if len > 0.0 {
            [q[0] / len, q[1] / len, q[2] / len, q[3] / len]
        } else {
            [0.0, 0.0, 0.0, 1.0]
        }
    }

    fn vec3_lerp(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
        let one_minus_t = 1.0 - t;
        [
            a[0] * one_minus_t + b[0] * t,
            a[1] * one_minus_t + b[1] * t,
            a[2] * one_minus_t + b[2] * t,
        ]
    }

    fn alpha_from_smoothing_factor(smoothing_factor: f32, dt_sec: Option<f32>) -> f32 {
        match dt_sec {
            Some(dt) if dt > 0.0 => 1.0 - (-smoothing_factor.max(0.0) * dt).exp(),
            _ => smoothing_factor.clamp(0.0, 1.0),
        }
    }

    fn debug_quat_filter_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            std::env::var("CAT_DEBUG_QUAT_FILTER")
                .ok()
                .map(|value| {
                    let value = value.trim().to_ascii_lowercase();
                    matches!(value.as_str(), "1" | "true" | "yes" | "on")
                })
                .unwrap_or(false)
        })
    }

    fn debug_quat_filter_window_len() -> usize {
        static WINDOW_LEN: OnceLock<usize> = OnceLock::new();
        *WINDOW_LEN.get_or_init(|| {
            std::env::var("CAT_DEBUG_QUAT_FILTER_WINDOW")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .map(|value| value.clamp(1, 600))
                .unwrap_or(60)
        })
    }

    fn push_rolling_sample(window: &mut VecDeque<f32>, value: f32, max_len: usize) {
        if window.len() >= max_len {
            let _ = window.pop_front();
        }
        window.push_back(value);
    }

    fn rolling_avg(window: &VecDeque<f32>) -> f32 {
        if window.is_empty() {
            0.0
        } else {
            window.iter().copied().sum::<f32>() / window.len() as f32
        }
    }

    fn rolling_max(window: &VecDeque<f32>) -> f32 {
        window.iter().copied().fold(0.0, f32::max)
    }

    fn quat_angle_degrees(a: [f32; 4], b: [f32; 4]) -> f32 {
        let a = Self::quat_normalize(a);
        let b = Self::quat_normalize(b);
        let dot = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3])
            .abs()
            .clamp(0.0, 1.0);
        (2.0 * dot.acos()).to_degrees()
    }
}

impl System for TransformOperatorSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        dt_sec: f32,
    ) {
        self.last_dt_sec = Some(dt_sec);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::World;
    use crate::engine::ecs::component::ComponentRef;

    fn guid_ref(world: &World, component: ComponentId) -> ComponentRef {
        ComponentRef::Guid(world.get_component_record(component).unwrap().guid)
    }

    #[test]
    fn translation_sample_masks_preserve_the_entire_linear_basis() {
        use crate::engine::ecs::component::transform_operator::TranslationAxes;
        let mut world = World::default();
        let source = world.add_component(TransformComponent::new());
        world
            .get_component_by_id_as_mut::<TransformComponent>(source)
            .unwrap()
            .transform
            .matrix_world[3] = [100.0, 200.0, 300.0, 1.0];
        let owner = world.add_component(TransformOperatorComponent::sample_translation(guid_ref(
            &world, source,
        )));
        // Include shear: sampling must not decompose/recompose the linear basis.
        let input = [
            [2.0, 0.3, 0.4, 0.0],
            [0.5, 3.0, 0.6, 0.0],
            [0.7, 0.8, 4.0, 0.0],
            [10.0, 20.0, 30.0, 1.0],
        ];
        let mut system = TransformOperatorSystem::new();
        for (mask, expected) in [
            ("y", [10.0, 200.0, 30.0, 1.0]),
            ("zx", [100.0, 20.0, 300.0, 1.0]),
            ("xyz", [100.0, 200.0, 300.0, 1.0]),
        ] {
            world
                .get_component_by_id_as_mut::<TransformOperatorComponent>(owner)
                .unwrap()
                .sample_translation_settings_mut()
                .unwrap()
                .axes = TranslationAxes::parse(mask).unwrap();
            let (output, _) = system.evaluate_operator(&world, owner, input).unwrap();
            assert_eq!(output[..3], input[..3]);
            assert_eq!(output[3], expected);
            assert_eq!(
                system.translation_sample_status(&world, owner),
                TranslationSampleStatus::Ready
            );
        }
        assert_eq!(TranslationAxes::default().as_str(), "xyz");
        assert_eq!(TranslationAxes::parse("zx").unwrap().as_str(), "xz");
        for invalid in ["", "yy", "w", "xyx", "X", "x y"] {
            assert!(TranslationAxes::parse(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn translation_sample_queries_cache_pose_changes_and_recover_after_topology_edits() {
        let mut world = World::default();
        let source = world.add_component(TransformComponent::new());
        world.get_component_record_mut(source).unwrap().name = "sample_source".into();
        let owner = world.add_component(TransformOperatorComponent::sample_translation(
            ComponentRef::Query("[name='sample_source']".into()),
        ));
        let input = crate::utils::math::mat4_identity();
        let mut system = TransformOperatorSystem::new();
        system.evaluate_operator(&world, owner, input).unwrap();
        let revision = world.query_revision();
        let relation_revision = system.sample_relation_revision;
        for value in 0..100 {
            world
                .get_component_by_id_as_mut::<TransformComponent>(source)
                .unwrap()
                .transform
                .matrix_world[3][1] = value as f32;
            assert_eq!(
                world.query_revision(),
                revision,
                "ordinary pose writes must retain selector caches"
            );
            let output = system.evaluate_operator(&world, owner, input).unwrap().0;
            assert_eq!(output[3][1], value as f32);
            assert_eq!(system.translation_samples[&owner].revision, revision);
            assert_eq!(system.sample_relation_revision, relation_revision);
        }
        let duplicate = world.add_component(TransformComponent::new());
        world.get_component_record_mut(duplicate).unwrap().name = "sample_source".into();
        assert_eq!(
            system.translation_sample_status(&world, owner),
            TranslationSampleStatus::Pending
        );
        assert_eq!(
            system.evaluate_operator(&world, owner, input).unwrap().0,
            input
        );
        assert_eq!(
            system.translation_sample_status(&world, owner),
            TranslationSampleStatus::AmbiguousSource
        );
        world.get_component_record_mut(duplicate).unwrap().name = "different".into();
        system.evaluate_operator(&world, owner, input).unwrap();
        assert_eq!(
            system.translation_sample_status(&world, owner),
            TranslationSampleStatus::Ready
        );
        system.component_removed(source);
        world.remove_component_leaf(source).unwrap();
        assert_eq!(
            system.evaluate_operator(&world, owner, input).unwrap().0,
            input
        );
        assert_eq!(
            system.translation_sample_status(&world, owner),
            TranslationSampleStatus::MissingSource
        );
        let replacement = world.add_component(TransformComponent::new());
        world.get_component_record_mut(replacement).unwrap().name = "sample_source".into();
        world
            .get_component_by_id_as_mut::<TransformComponent>(replacement)
            .unwrap()
            .transform
            .matrix_world[3][1] = 42.0;
        assert_eq!(
            system.evaluate_operator(&world, owner, input).unwrap().0[3][1],
            42.0
        );
        assert_eq!(system.translation_samples[&owner].source, Some(replacement));
        system.component_removed(owner);
        assert!(system.translation_samples.is_empty());
        assert!(system.translation_dependents.is_empty());
    }

    #[test]
    fn translation_sample_invalid_targets_nonfinite_values_and_cycles_preserve_input() {
        use crate::engine::ecs::component::transform_operator::TranslationAxes;
        let mut world = World::default();
        let source = world.add_component(TransformComponent::new());
        let owner = world.add_component(
            TransformOperatorComponent::sample_translation(guid_ref(&world, source))
                .with_axes(TranslationAxes::Y),
        );
        let input = TransformComponent::new()
            .with_position(10.0, 20.0, 30.0)
            .transform
            .model;
        let mut system = TransformOperatorSystem::new();
        world
            .get_component_by_id_as_mut::<TransformComponent>(source)
            .unwrap()
            .transform
            .matrix_world[3][1] = f32::NAN;
        assert_eq!(
            system.evaluate_operator(&world, owner, input).unwrap().0,
            input
        );
        assert_eq!(
            system.translation_sample_status(&world, owner),
            TranslationSampleStatus::NonFinite
        );
        world
            .get_component_by_id_as_mut::<TransformComponent>(source)
            .unwrap()
            .transform
            .matrix_world[3][1] = 7.0;
        assert_eq!(
            system.evaluate_operator(&world, owner, input).unwrap().0[3][1],
            7.0
        );
        // Sampling one's own output creates feedback even without a second sampler.
        world.add_child(owner, source).unwrap();
        assert_eq!(
            system.evaluate_operator(&world, owner, input).unwrap().0,
            input
        );
        assert_eq!(
            system.translation_sample_status(&world, owner),
            TranslationSampleStatus::DependencyCycle
        );
        let wrong = guid_ref(&world, owner);
        world
            .get_component_by_id_as_mut::<TransformOperatorComponent>(owner)
            .unwrap()
            .sample_translation_settings_mut()
            .unwrap()
            .source = Some(wrong);
        assert_eq!(
            system.evaluate_operator(&world, owner, input).unwrap().0,
            input
        );
        assert_eq!(
            system.translation_sample_status(&world, owner),
            TranslationSampleStatus::InvalidTarget
        );
    }

    #[test]
    fn translation_sample_scoped_queries_and_cross_branch_cycles_are_validated() {
        let mut world = World::default();
        let root = world.add_component(TransformComponent::new());
        let source = world.add_component(TransformComponent::new());
        world.get_component_record_mut(source).unwrap().name = "sample_source".into();
        world.add_child(root, source).unwrap();
        let outside = world.add_component(TransformComponent::new());
        world.get_component_record_mut(outside).unwrap().name = "sample_source".into();
        let owner = world.add_component(
            TransformOperatorComponent::sample_translation(ComponentRef::Query(
                "[name='sample_source']".into(),
            ))
            .with_root_source(guid_ref(&world, root)),
        );
        let input = crate::utils::math::mat4_identity();
        let mut system = TransformOperatorSystem::new();
        system.evaluate_operator(&world, owner, input).unwrap();
        assert_eq!(system.translation_samples[&owner].source, Some(source));
        let settings = world
            .get_component_by_id_as_mut::<TransformOperatorComponent>(owner)
            .unwrap()
            .sample_translation_settings_mut()
            .unwrap();
        settings.source = Some(ComponentRef::Query("/> [name='sample_source']".into()));
        system.evaluate_operator(&world, owner, input).unwrap();
        assert_eq!(system.translation_samples[&owner].source, Some(outside));
        // A <- B <- A across sibling subtrees. Both must reject after settling.
        let a = world.add_component(TransformComponent::new());
        let b = world.add_component(TransformComponent::new());
        let op_a = world.add_component(TransformOperatorComponent::sample_translation(guid_ref(
            &world, b,
        )));
        let op_b = world.add_component(TransformOperatorComponent::sample_translation(guid_ref(
            &world, a,
        )));
        world.add_child(op_a, a).unwrap();
        world.add_child(op_b, b).unwrap();
        for _ in 0..2 {
            system.evaluate_operator(&world, op_a, input).unwrap();
            system.evaluate_operator(&world, op_b, input).unwrap();
        }
        assert_eq!(
            system.translation_sample_status(&world, op_a),
            TranslationSampleStatus::DependencyCycle
        );
        assert_eq!(
            system.translation_sample_status(&world, op_b),
            TranslationSampleStatus::DependencyCycle
        );
    }

    #[test]
    fn translation_sample_revalidates_cycles_after_live_parent_source_edits() {
        let mut world = World::default();
        let external = world.add_component(TransformComponent::new());
        let route = world.add_component(
            TransformOperatorComponent::parent().with_source(guid_ref(&world, external)),
        );
        let source = world.add_component(TransformComponent::new());
        world.add_child(route, source).unwrap();
        let owner = world.add_component(TransformOperatorComponent::sample_translation(guid_ref(
            &world, source,
        )));
        let output = world.add_component(TransformComponent::new());
        world.add_child(owner, output).unwrap();
        let input = crate::utils::math::mat4_identity();
        let mut system = TransformOperatorSystem::new();
        system.evaluate_operator(&world, route, input).unwrap();
        system.evaluate_operator(&world, owner, input).unwrap();
        assert_eq!(
            system.translation_sample_status(&world, owner),
            TranslationSampleStatus::Ready
        );
        let revision = world.query_revision();
        let changed = guid_ref(&world, output);
        world
            .get_component_by_id_as_mut::<TransformOperatorComponent>(route)
            .unwrap()
            .parent_settings_mut()
            .unwrap()
            .target_source = Some(changed);
        assert_eq!(world.query_revision(), revision);
        system.evaluate_operator(&world, route, input).unwrap();
        system.evaluate_operator(&world, owner, input).unwrap();
        assert_eq!(
            system.translation_sample_status(&world, owner),
            TranslationSampleStatus::DependencyCycle
        );
        let restored = guid_ref(&world, external);
        world
            .get_component_by_id_as_mut::<TransformOperatorComponent>(route)
            .unwrap()
            .parent_settings_mut()
            .unwrap()
            .target_source = Some(restored);
        system.evaluate_operator(&world, route, input).unwrap();
        system.evaluate_operator(&world, owner, input).unwrap();
        assert_eq!(
            system.translation_sample_status(&world, owner),
            TranslationSampleStatus::Ready
        );
    }

    #[test]
    fn smoothing_translation_and_scale_keeps_independent_channel_state() {
        let mut world = World::default();
        let mut operator = TransformOperatorComponent::fork_trs();
        for channel in [TransformChannel::Translation, TransformChannel::Scale] {
            operator
                .push_vec3(
                    channel,
                    TransformVec3Op::TemporalSmooth {
                        smoothing_factor: 12.0,
                    },
                )
                .unwrap();
        }
        let owner = world.add_component(operator);
        let mut system = TransformOperatorSystem::new();
        let plan = system.plan_for_operator(&world, owner).unwrap();
        let first_input = TransformComponent::new()
            .with_position(10.0, 20.0, 30.0)
            .with_scale(2.0, 3.0, 4.0)
            .transform
            .model;
        let first = system.evaluate_plan(&plan, first_input, &world, Some(1.0 / 60.0));
        assert_eq!(first, first_input);
        let next_input = TransformComponent::new()
            .with_position(20.0, 30.0, 40.0)
            .with_scale(4.0, 6.0, 8.0)
            .transform
            .model;
        let next = system.evaluate_plan(&plan, next_input, &world, Some(1.0 / 60.0));
        let alpha = TransformOperatorSystem::alpha_from_smoothing_factor(12.0, Some(1.0 / 60.0));
        for axis in 0..3 {
            let expected_translation =
                first_input[3][axis] + (next_input[3][axis] - first_input[3][axis]) * alpha;
            let expected_scale = first_input[axis][axis]
                + (next_input[axis][axis] - first_input[axis][axis]) * alpha;
            assert!((next[3][axis] - expected_translation).abs() < 1e-5);
            assert!((next[axis][axis] - expected_scale).abs() < 1e-5);
        }
    }

    #[test]
    fn equal_settings_keep_separate_state_for_live_operators() {
        let mut world = World::default();
        let mut settings = TransformOperatorComponent::fork_trs();
        settings
            .push_vec3(
                TransformChannel::Translation,
                TransformVec3Op::TemporalSmooth {
                    smoothing_factor: 12.0,
                },
            )
            .unwrap();
        let first = world.add_component(settings.clone());
        let second = world.add_component(settings);
        let mut system = TransformOperatorSystem::new();
        system.last_dt_sec = Some(1.0 / 60.0);
        let input_a = TransformComponent::new()
            .with_position(10.0, 0.0, 0.0)
            .transform
            .model;
        let input_b = TransformComponent::new()
            .with_position(-10.0, 0.0, 0.0)
            .transform
            .model;
        assert_eq!(
            system.evaluate_operator(&world, first, input_a).unwrap().0,
            input_a
        );
        assert_eq!(
            system.evaluate_operator(&world, second, input_b).unwrap().0,
            input_b
        );
        let next = TransformComponent::new()
            .with_position(20.0, 0.0, 0.0)
            .transform
            .model;
        let actual = system.evaluate_operator(&world, first, next).unwrap().0;
        let alpha = TransformOperatorSystem::alpha_from_smoothing_factor(12.0, Some(1.0 / 60.0));
        assert!((actual[3][0] - (10.0 + 10.0 * alpha)).abs() < 1e-5);
    }

    #[test]
    fn inverse_local_applies_full_inverse_and_returns_only_structural_children() {
        let mut world = World::default();
        let source = world.add_component(
            TransformComponent::new()
                .with_position(1.0, 2.0, 3.0)
                .with_rotation_euler(0.0, std::f32::consts::FRAC_PI_2, 0.0),
        );
        let operator = world.add_component(TransformOperatorComponent::inverse_local(guid_ref(
            &world, source,
        )));
        let downstream = world.add_component(TransformComponent::new());
        let sibling = world.add_component(TransformComponent::new());
        world.add_child(operator, downstream).unwrap();

        let input = TransformComponent::new()
            .with_position(10.0, 20.0, 30.0)
            .transform
            .model;
        let source_local = world
            .get_component_by_id_as::<TransformComponent>(source)
            .unwrap()
            .transform
            .model;
        let expected = math::mat4_mul(input, math::mat4_inverse(source_local).unwrap());

        let (actual, outputs) = TransformOperatorSystem::new()
            .evaluate_operator(&world, operator, input)
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(outputs, vec![downstream]);
        assert!(!outputs.contains(&sibling));
    }

    #[test]
    fn inverse_local_preserves_last_valid_result_when_source_becomes_unresolved() {
        let mut world = World::default();
        let source = world.add_component(TransformComponent::new().with_position(0.0, 3.0, -1.55));
        let source_ref = guid_ref(&world, source);
        let operator = world.add_component(TransformOperatorComponent::inverse_local(source_ref));
        let downstream = world.add_component(TransformComponent::new());
        world.add_child(operator, downstream).unwrap();
        let input = TransformComponent::new().transform.model;
        let mut system = TransformOperatorSystem::new();

        let first = system.evaluate_operator(&world, operator, input).unwrap();
        world.remove_component_leaf(source).unwrap();
        let unresolved = system.evaluate_operator(&world, operator, input).unwrap();

        assert_eq!(unresolved, first);
    }

    #[test]
    fn inverse_local_rejects_a_source_below_its_output() {
        let mut world = World::default();
        let source = world.add_component(TransformComponent::new().with_position(0.0, 3.0, 0.0));
        let operator = world.add_component(TransformOperatorComponent::inverse_local(guid_ref(
            &world, source,
        )));
        world.add_child(operator, source).unwrap();
        let input = TransformComponent::new().transform.model;

        let (actual, outputs) = TransformOperatorSystem::new()
            .evaluate_operator(&world, operator, input)
            .unwrap();
        assert_eq!(actual, input);
        assert!(outputs.is_empty());
    }

    #[test]
    fn inverse_local_dependency_index_rebinds_without_a_world_scan() {
        let mut world = World::default();
        let first_source = world.add_component(TransformComponent::new());
        let second_source = world.add_component(TransformComponent::new());
        let operator = world.add_component(TransformOperatorComponent::inverse_local(guid_ref(
            &world,
            first_source,
        )));
        let input = TransformComponent::new().transform.model;
        let mut system = TransformOperatorSystem::new();

        system.evaluate_operator(&world, operator, input).unwrap();
        assert_eq!(
            system.inverse_local_dependents(first_source),
            vec![operator]
        );

        let second_ref = guid_ref(&world, second_source);
        world
            .get_component_by_id_as_mut::<TransformOperatorComponent>(operator)
            .and_then(|operator| operator.inverse_local_settings_mut())
            .unwrap()
            .source = second_ref;
        system.evaluate_operator(&world, operator, input).unwrap();

        assert!(system.inverse_local_dependents(first_source).is_empty());
        assert_eq!(
            system.inverse_local_dependents(second_source),
            vec![operator]
        );
    }

    #[test]
    fn camera_specific_selects_modes_and_excludes_configuration_children() {
        let mut world = World::default();
        let anchor = world.add_component(TransformComponent::new());
        let mono = world.add_component(TransformOperatorComponent::camera_specific(
            TransformCameraSpecificMode::Monoscopic,
        ));
        let mono_settings =
            world.add_component(TransformComponent::new().with_position(0.0, 2.0, 0.0));
        let stereo = world.add_component(TransformOperatorComponent::camera_specific(
            TransformCameraSpecificMode::Stereoscopic,
        ));
        let stereo_settings =
            world.add_component(TransformComponent::new().with_position(0.0, 0.0, 3.0));
        let output = world.add_component(TransformComponent::new());
        world.add_child(anchor, mono).unwrap();
        world.add_child(mono, mono_settings).unwrap();
        world.add_child(anchor, stereo).unwrap();
        world.add_child(stereo, stereo_settings).unwrap();
        world.add_child(anchor, output).unwrap();
        let input = TransformComponent::new()
            .with_position(1.0, 0.0, 0.0)
            .transform
            .model;
        let mut system = TransformOperatorSystem::new();
        let (m, outputs) = system.evaluate_operator(&world, anchor, input).unwrap();
        assert_eq!([m[3][0], m[3][1], m[3][2]], [1.0, 2.0, 0.0]);
        assert_eq!(outputs, vec![output]);
        system.set_stereoscopic_active(true);
        let (m, outputs) = system.evaluate_operator(&world, anchor, input).unwrap();
        assert_eq!([m[3][0], m[3][1], m[3][2]], [1.0, 0.0, 3.0]);
        assert_eq!(outputs, vec![output]);
    }

    #[test]
    fn camera_specific_uses_generic_anchor_when_mode_has_no_settings() {
        let mut world = World::default();
        let anchor = world.add_component(TransformComponent::new());
        let mono = world.add_component(TransformOperatorComponent::camera_specific(
            TransformCameraSpecificMode::Monoscopic,
        ));
        let settings = world.add_component(TransformComponent::new().with_scale(2.0, 2.0, 2.0));
        world.add_child(anchor, mono).unwrap();
        world.add_child(mono, settings).unwrap();
        let input = TransformComponent::new()
            .with_position(4.0, 5.0, 6.0)
            .transform
            .model;
        let mut system = TransformOperatorSystem::new();
        system.set_stereoscopic_active(true);
        assert_eq!(
            system.evaluate_operator(&world, anchor, input).unwrap().0,
            input
        );
    }

    #[test]
    fn repeated_camera_specific_refresh_does_not_compound_scale() {
        let mut world = World::default();
        let anchor = world.add_component(TransformComponent::new());
        let mono = world.add_component(TransformOperatorComponent::camera_specific(
            TransformCameraSpecificMode::Monoscopic,
        ));
        let settings = world.add_component(TransformComponent::new().with_scale(2.0, 2.0, 2.0));
        world.add_child(anchor, mono).unwrap();
        world.add_child(mono, settings).unwrap();

        let mut system = TransformOperatorSystem::new();
        let identity = TransformComponent::new().transform.model;
        let first = system
            .evaluate_operator(&world, anchor, identity)
            .unwrap()
            .0;
        let second = system.evaluate_operator(&world, anchor, first).unwrap().0;
        let third = system.evaluate_operator(&world, anchor, second).unwrap().0;

        assert_eq!(first, second);
        assert_eq!(second, third);
        assert_eq!(third[0][0], 2.0);
        assert_eq!(third[1][1], 2.0);
        assert_eq!(third[2][2], 2.0);
    }

    #[test]
    fn controller_rotation_operator_contains_temporal_quat_op() {
        let block = TransformOperatorSystem::controller_rotation_smoothing_plan(None, 1.0);
        let stage = match &block.stages[0] {
            TransformOperatorStage::ForkTrs(stage) => stage,
        };
        assert_eq!(
            stage.rotation_ops,
            vec![TransformQuatOp::TemporalFilter {
                smoothing_factor: 1.0
            }]
        );
    }

    #[test]
    fn ordinary_fork_children_are_all_outputs() {
        let mut world = World::default();
        let fork = world.add_component(TransformOperatorComponent::fork_trs());
        let nested = world.add_component(TransformOperatorComponent::fork_trs());
        let downstream = world.add_component(TransformComponent::new());
        world.add_child(fork, nested).unwrap();
        world.add_child(fork, downstream).unwrap();
        let input = TransformComponent::new()
            .with_position(2.0, 3.0, 4.0)
            .transform
            .model;
        let (output, children) = TransformOperatorSystem::new()
            .evaluate_operator(&world, fork, input)
            .unwrap();
        assert_eq!(output, input);
        assert_eq!(children, vec![nested, downstream]);
    }
}
