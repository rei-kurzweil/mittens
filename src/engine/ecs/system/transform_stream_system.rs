use crate::engine::ecs::component::{
    QuatExtractYawComponent, QuatTemporalFilterComponent, QuatYawFollowComponent,
    TransformComponent, TransformDropComponent, TransformForkTRSComponent,
    TransformMapRotationComponent, TransformMapScaleComponent, TransformMapTranslationComponent,
    TransformMergeTRSComponent, TransformParentComponent, TransformSampleAncestorComponent,
    Vector3TemporalFilterComponent,
};
use crate::engine::ecs::system::System;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::VisualWorld;
use crate::engine::graphics::primitives::TransformMatrix;
use crate::engine::user_input::InputState;
use crate::utils::math;
use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformPipelineInput {
    ParentWorld,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransformPipelineVec3Op {
    Pass,
    Drop,
    TemporalSmooth {
        smoothing_factor: f32,
    },
    /// Replace this channel's value with the world translation of an ancestor
    /// TransformComponent. `skip` counts TransformComponent ancestors upward from the
    /// pipeline owner: 0 = the driven T directly above the pipeline, 1 = the next T above
    /// that (e.g. the armature bone above an InputXR splice), etc.
    SampleAncestorTranslation {
        skip: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransformPipelineQuatOp {
    Pass,
    Drop,
    TemporalFilter {
        smoothing_factor: f32,
    },
    /// Replace this channel's value with the world rotation of an ancestor
    /// TransformComponent. Same `skip` semantics as `SampleAncestorTranslation`.
    SampleAncestorRotation {
        skip: usize,
    },
    /// Project the rotation onto the Y-rotation subspace: `normalize([0, q.y, 0, q.w])`.
    /// Strips pitch and roll, keeping only the Y-axis component. Convention-independent.
    ExtractYaw,
    /// Stateful body-yaw follow. Extracts world-Y yaw from the input quaternion using
    /// the specified forward convention, then advances a running `body_yaw` toward the
    /// extracted head yaw when the delta exceeds `threshold`, at `rate` rad/s.
    /// Outputs a pure-Y quaternion for `body_yaw`.
    YawFollow {
        threshold: f32,
        rate: f32,
        initial_yaw: f32,
        forward_plus_z: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransformForkTrsStage {
    pub translation_ops: Vec<TransformPipelineVec3Op>,
    pub rotation_ops: Vec<TransformPipelineQuatOp>,
    pub scale_ops: Vec<TransformPipelineVec3Op>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransformPipelineStage {
    ForkTrs(TransformForkTrsStage),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransformPipelinePlan {
    pub owner_component: Option<ComponentId>,
    pub input: TransformPipelineInput,
    pub stages: Vec<TransformPipelineStage>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformPipelineChannels {
    pub translation: [f32; 3],
    pub rotation_quat_xyzw: [f32; 4],
    pub scale: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TransformPipelineStageKey {
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

#[derive(Debug, Default)]
pub struct TransformStreamSystem {
    last_dt_sec: Option<f32>,
    vec3_temporal_state: HashMap<TransformPipelineStageKey, Vec3TemporalState>,
    quat_temporal_state: HashMap<TransformPipelineStageKey, QuatTemporalState>,
    yaw_follow_state: HashMap<TransformPipelineStageKey, f32>,
}

impl TransformStreamSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_transform_stream_boundary(&self, world: &World, cid: ComponentId) -> bool {
        world
            .get_component_by_id_as::<TransformForkTRSComponent>(cid)
            .is_some()
            || world
                .get_component_by_id_as::<TransformParentComponent>(cid)
                .is_some()
    }

    pub fn parse_component_tree(
        &self,
        world: &World,
        root: ComponentId,
    ) -> Option<TransformPipelinePlan> {
        if world
            .get_component_by_id_as::<TransformForkTRSComponent>(root)
            .is_some()
        {
            return Some(TransformPipelinePlan {
                owner_component: Some(root),
                input: TransformPipelineInput::ParentWorld,
                stages: vec![TransformPipelineStage::ForkTrs(
                    self.parse_fork_trs(world, root),
                )],
            });
        }
        None
    }

    pub fn evaluate_stream_node(
        &mut self,
        world: &World,
        root: ComponentId,
        input_world: TransformMatrix,
    ) -> Option<(TransformMatrix, Vec<ComponentId>)> {
        let rebased_world = Self::apply_transform_parent_basis(world, root, input_world);

        if let Some(block) = self.parse_component_tree(world, root) {
            let outputs = self.downstream_children(world, root, &block);
            let world_matrix = self.evaluate_block(&block, rebased_world, world, self.last_dt_sec);
            return Some((world_matrix, outputs));
        }

        if world
            .get_component_by_id_as::<TransformParentComponent>(root)
            .is_some()
        {
            return Some((rebased_world, world.children_of(root).to_vec()));
        }

        None
    }

    fn apply_transform_parent_basis(
        world: &World,
        node: ComponentId,
        current_world: TransformMatrix,
    ) -> TransformMatrix {
        world
            .get_component_by_id_as::<TransformParentComponent>(node)
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

    fn downstream_children(
        &self,
        world: &World,
        root: ComponentId,
        _block: &TransformPipelinePlan,
    ) -> Vec<ComponentId> {
        world
            .children_of(root)
            .iter()
            .copied()
            .filter(|&child| {
                world
                    .get_component_by_id_as::<TransformMapTranslationComponent>(child)
                    .is_none()
                    && world
                        .get_component_by_id_as::<TransformMapRotationComponent>(child)
                        .is_none()
                    && world
                        .get_component_by_id_as::<TransformMapScaleComponent>(child)
                        .is_none()
                    && world
                        .get_component_by_id_as::<TransformMergeTRSComponent>(child)
                        .is_none()
            })
            .collect()
    }

    pub fn pipeline_for_controller_rotation_smoothing(
        owner_component: Option<ComponentId>,
        smoothing_factor: f32,
    ) -> TransformPipelinePlan {
        TransformPipelinePlan {
            owner_component,
            input: TransformPipelineInput::ParentWorld,
            stages: vec![TransformPipelineStage::ForkTrs(TransformForkTrsStage {
                translation_ops: vec![TransformPipelineVec3Op::Pass],
                rotation_ops: vec![TransformPipelineQuatOp::TemporalFilter { smoothing_factor }],
                scale_ops: vec![TransformPipelineVec3Op::Pass],
            })],
        }
    }

    fn parse_fork_trs(&self, world: &World, node: ComponentId) -> TransformForkTrsStage {
        let mut translation_ops = vec![TransformPipelineVec3Op::Pass];
        let mut rotation_ops = vec![TransformPipelineQuatOp::Pass];
        let mut scale_ops = vec![TransformPipelineVec3Op::Pass];

        for &child in world.children_of(node) {
            if world
                .get_component_by_id_as::<TransformMapTranslationComponent>(child)
                .is_some()
            {
                translation_ops = self.parse_vec3_ops(world, child);
                continue;
            }

            if world
                .get_component_by_id_as::<TransformMapRotationComponent>(child)
                .is_some()
            {
                rotation_ops = self.parse_quat_ops(world, child);
                continue;
            }

            if world
                .get_component_by_id_as::<TransformMapScaleComponent>(child)
                .is_some()
            {
                scale_ops = self.parse_vec3_ops(world, child);
                continue;
            }

            if world
                .get_component_by_id_as::<TransformMergeTRSComponent>(child)
                .is_some()
            {
                debug_assert!(
                    world.children_of(child).is_empty(),
                    "TransformMergeTRS should not contain child stages; attach driven children directly under the TransformForkTRS root"
                );
                continue;
            }
        }

        TransformForkTrsStage {
            translation_ops,
            rotation_ops,
            scale_ops,
        }
    }

    fn parse_vec3_ops(&self, world: &World, node: ComponentId) -> Vec<TransformPipelineVec3Op> {
        let mut ops = Vec::new();
        for &child in world.children_of(node) {
            if world
                .get_component_by_id_as::<TransformDropComponent>(child)
                .is_some()
            {
                ops.push(TransformPipelineVec3Op::Drop);
                continue;
            }
            if let Some(s) = world.get_component_by_id_as::<TransformSampleAncestorComponent>(child)
            {
                ops.push(TransformPipelineVec3Op::SampleAncestorTranslation { skip: s.skip });
                continue;
            }
            if let Some(filter) =
                world.get_component_by_id_as::<Vector3TemporalFilterComponent>(child)
            {
                ops.push(TransformPipelineVec3Op::TemporalSmooth {
                    smoothing_factor: filter.smoothing_factor,
                });
            }
        }
        if ops.is_empty() {
            ops.push(TransformPipelineVec3Op::Pass);
        }
        ops
    }

    fn parse_quat_ops(&self, world: &World, node: ComponentId) -> Vec<TransformPipelineQuatOp> {
        let mut ops = Vec::new();
        for &child in world.children_of(node) {
            if world
                .get_component_by_id_as::<TransformDropComponent>(child)
                .is_some()
            {
                ops.push(TransformPipelineQuatOp::Drop);
                continue;
            }
            if let Some(s) = world.get_component_by_id_as::<TransformSampleAncestorComponent>(child)
            {
                ops.push(TransformPipelineQuatOp::SampleAncestorRotation { skip: s.skip });
                continue;
            }
            if let Some(filter) = world.get_component_by_id_as::<QuatTemporalFilterComponent>(child)
            {
                ops.push(TransformPipelineQuatOp::TemporalFilter {
                    smoothing_factor: filter.smoothing_factor,
                });
                continue;
            }
            if world
                .get_component_by_id_as::<QuatExtractYawComponent>(child)
                .is_some()
            {
                ops.push(TransformPipelineQuatOp::ExtractYaw);
                continue;
            }
            if let Some(c) = world.get_component_by_id_as::<QuatYawFollowComponent>(child) {
                ops.push(TransformPipelineQuatOp::YawFollow {
                    threshold: c.threshold,
                    rate: c.rate,
                    initial_yaw: c.initial_yaw,
                    forward_plus_z: c.forward_plus_z,
                });
            }
        }
        if ops.is_empty() {
            ops.push(TransformPipelineQuatOp::Pass);
        }
        ops
    }

    pub fn evaluate_block(
        &mut self,
        pipeline: &TransformPipelinePlan,
        input_world: TransformMatrix,
        world: &World,
        dt_sec: Option<f32>,
    ) -> TransformMatrix {
        let mut channels = Self::decompose_matrix(input_world);
        for (stage_index, stage) in pipeline.stages.iter().enumerate() {
            let mut stage_path = vec![stage_index];
            channels = self.evaluate_stage(
                pipeline.owner_component,
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
        stage: &TransformPipelineStage,
        input: TransformPipelineChannels,
        stage_path: &mut Vec<usize>,
        world: &World,
        dt_sec: Option<f32>,
    ) -> TransformPipelineChannels {
        match stage {
            TransformPipelineStage::ForkTrs(fork) => {
                self.evaluate_fork_trs(owner_component, fork, input, stage_path, world, dt_sec)
            }
        }
    }

    fn evaluate_fork_trs(
        &mut self,
        owner_component: Option<ComponentId>,
        fork: &TransformForkTrsStage,
        input: TransformPipelineChannels,
        stage_path: &[usize],
        world: &World,
        dt_sec: Option<f32>,
    ) -> TransformPipelineChannels {
        let translation = self.apply_vec3_ops(
            owner_component,
            &fork.translation_ops,
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
            input.scale,
            [1.0, 1.0, 1.0],
            stage_path,
            world,
            dt_sec,
        );

        TransformPipelineChannels {
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
        ops: &[TransformPipelineVec3Op],
        input: [f32; 3],
        dropped_value: [f32; 3],
        stage_path: &[usize],
        world: &World,
        dt_sec: Option<f32>,
    ) -> [f32; 3] {
        let mut current = input;
        for (op_index, op) in ops.iter().enumerate() {
            current = match *op {
                TransformPipelineVec3Op::Pass => current,
                TransformPipelineVec3Op::Drop => dropped_value,
                TransformPipelineVec3Op::SampleAncestorTranslation { skip } => owner_component
                    .and_then(|owner| Self::sample_ancestor_world(world, owner, skip))
                    .map(|m| [m[3][0], m[3][1], m[3][2]])
                    .unwrap_or(current),
                TransformPipelineVec3Op::TemporalSmooth { smoothing_factor } => {
                    let mut full_path = stage_path.to_vec();
                    full_path.push(op_index);
                    let key = TransformPipelineStageKey {
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
        ops: &[TransformPipelineQuatOp],
        input: [f32; 4],
        dropped_value: [f32; 4],
        stage_path: &[usize],
        world: &World,
        dt_sec: Option<f32>,
    ) -> [f32; 4] {
        let mut current = Self::quat_normalize(input);
        for (op_index, op) in ops.iter().enumerate() {
            current = match *op {
                TransformPipelineQuatOp::Pass => current,
                TransformPipelineQuatOp::Drop => dropped_value,
                TransformPipelineQuatOp::SampleAncestorRotation { skip } => owner_component
                    .and_then(|owner| Self::sample_ancestor_world(world, owner, skip))
                    .map(|m| Self::decompose_matrix(m).rotation_quat_xyzw)
                    .unwrap_or(current),
                TransformPipelineQuatOp::ExtractYaw => {
                    // Project onto Y-rotation subspace: normalize([0, q.y, 0, q.w])
                    let (qy, qw) = (current[1], current[3]);
                    let len = (qy * qy + qw * qw).sqrt().max(1e-8);
                    [0.0, qy / len, 0.0, qw / len]
                }
                TransformPipelineQuatOp::YawFollow {
                    threshold,
                    rate,
                    initial_yaw,
                    forward_plus_z,
                } => {
                    let mut full_path = stage_path.to_vec();
                    full_path.push(op_index);
                    let key = TransformPipelineStageKey {
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
                TransformPipelineQuatOp::TemporalFilter { smoothing_factor } => {
                    let mut full_path = stage_path.to_vec();
                    full_path.push(op_index);
                    let key = TransformPipelineStageKey {
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
                                "[TransformPipeline][QuatFilter] owner={owner_component:?} stage_path={full_path:?} smoothing_factor={smoothing_factor:.3} dt={:.5} alpha={alpha:.5} raw_avg_deg={avg_raw:.3} filtered_avg_deg={avg_filtered:.3} lag_avg_deg={avg_lag:.3} raw_max_deg={max_raw:.3} filtered_max_deg={max_filtered:.3} attenuation_pct={attenuation_pct:.1} window={} samples={}",
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

    fn decompose_matrix(m: TransformMatrix) -> TransformPipelineChannels {
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

        TransformPipelineChannels {
            translation,
            rotation_quat_xyzw,
            scale,
        }
    }

    fn recompose_matrix(channels: TransformPipelineChannels) -> TransformMatrix {
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

impl From<TransformMatrix> for TransformPipelineChannels {
    fn from(value: TransformMatrix) -> Self {
        TransformStreamSystem::decompose_matrix(value)
    }
}

impl System for TransformStreamSystem {
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
    use crate::engine::ecs::component::{
        QuatTemporalFilterComponent, TransformForkTRSComponent, TransformMapRotationComponent,
        TransformMapScaleComponent, TransformMapTranslationComponent,
        Vector3TemporalFilterComponent,
    };

    #[test]
    fn controller_rotation_pipeline_contains_temporal_quat_op() {
        let block = TransformStreamSystem::pipeline_for_controller_rotation_smoothing(None, 1.0);
        let stage = match &block.stages[0] {
            TransformPipelineStage::ForkTrs(stage) => stage,
        };
        assert_eq!(
            stage.rotation_ops,
            vec![TransformPipelineQuatOp::TemporalFilter {
                smoothing_factor: 1.0
            }]
        );
    }

    #[test]
    fn parses_fork_root_component_tree() {
        let mut world = World::default();
        let fork = world.add_component(TransformForkTRSComponent::new());
        let map_translation = world.add_component(TransformMapTranslationComponent::new());
        let vec_filter =
            world.add_component(Vector3TemporalFilterComponent::new().with_smoothing_factor(12.0));
        let map_rotation = world.add_component(TransformMapRotationComponent::new());
        let quat_filter =
            world.add_component(QuatTemporalFilterComponent::new().with_smoothing_factor(18.0));

        world.set_parent(map_translation, Some(fork)).unwrap();
        world.set_parent(vec_filter, Some(map_translation)).unwrap();
        world.set_parent(map_rotation, Some(fork)).unwrap();
        world.set_parent(quat_filter, Some(map_rotation)).unwrap();

        let parser = TransformStreamSystem::new();
        let block = parser
            .parse_component_tree(&world, fork)
            .expect("pipeline block");
        assert_eq!(block.owner_component, Some(fork));
        assert_eq!(block.stages.len(), 1);
        let stage = match &block.stages[0] {
            TransformPipelineStage::ForkTrs(stage) => stage,
        };
        assert_eq!(
            stage.translation_ops,
            vec![TransformPipelineVec3Op::TemporalSmooth {
                smoothing_factor: 12.0
            }]
        );
        assert_eq!(
            stage.rotation_ops,
            vec![TransformPipelineQuatOp::TemporalFilter {
                smoothing_factor: 18.0
            }]
        );
        assert_eq!(stage.scale_ops, vec![TransformPipelineVec3Op::Pass]);
    }

    #[test]
    fn fork_trs_defaults_missing_streams_to_pass() {
        let mut world = World::default();
        let fork = world.add_component(TransformForkTRSComponent::new());
        let map_scale = world.add_component(TransformMapScaleComponent::new());

        world.set_parent(map_scale, Some(fork)).unwrap();

        let parser = TransformStreamSystem::new();
        let block = parser
            .parse_component_tree(&world, fork)
            .expect("pipeline block");
        let stage = match &block.stages[0] {
            TransformPipelineStage::ForkTrs(stage) => stage,
        };

        assert_eq!(stage.translation_ops, vec![TransformPipelineVec3Op::Pass]);
        assert_eq!(stage.rotation_ops, vec![TransformPipelineQuatOp::Pass]);
        assert_eq!(stage.scale_ops, vec![TransformPipelineVec3Op::Pass]);
    }

    #[test]
    fn parses_fork_trs_as_root_pipeline() {
        let mut world = World::default();
        let fork = world.add_component(TransformForkTRSComponent::new());
        let map_rotation = world.add_component(TransformMapRotationComponent::new());
        let quat_filter =
            world.add_component(QuatTemporalFilterComponent::new().with_smoothing_factor(9.0));

        world.set_parent(map_rotation, Some(fork)).unwrap();
        world.set_parent(quat_filter, Some(map_rotation)).unwrap();

        let parser = TransformStreamSystem::new();
        let block = parser
            .parse_component_tree(&world, fork)
            .expect("fork root block");
        assert_eq!(block.owner_component, Some(fork));
        assert_eq!(block.stages.len(), 1);
        let stage = match &block.stages[0] {
            TransformPipelineStage::ForkTrs(stage) => stage,
        };
        assert_eq!(
            stage.rotation_ops,
            vec![TransformPipelineQuatOp::TemporalFilter {
                smoothing_factor: 9.0
            }]
        );
    }

    #[test]
    fn fork_root_returns_non_map_children_as_downstream_children() {
        let mut world = World::default();
        let fork = world.add_component(TransformForkTRSComponent::new());
        let map_rotation = world.add_component(TransformMapRotationComponent::new());
        let quat_filter =
            world.add_component(QuatTemporalFilterComponent::new().with_smoothing_factor(9.0));
        let downstream = world.add_component(TransformComponent::new());

        world.set_parent(map_rotation, Some(fork)).unwrap();
        world.set_parent(quat_filter, Some(map_rotation)).unwrap();
        world.set_parent(downstream, Some(fork)).unwrap();

        let mut system = TransformStreamSystem::new();
        let ident = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let (_, children) = system
            .evaluate_stream_node(&world, fork, ident)
            .expect("fork root eval");
        assert_eq!(children, vec![downstream]);
    }
}
