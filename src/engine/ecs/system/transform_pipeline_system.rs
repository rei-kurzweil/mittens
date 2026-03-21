use crate::engine::ecs::component::{
    QuatTemporalFilterComponent, TransformComponent, TransformDropComponent,
    TransformForkTRSComponent, TransformMapRotationComponent, TransformMapScaleComponent,
    TransformMapTranslationComponent, TransformMergeTRSComponent, TransformPipelineComponent,
    TransformPipelineOutputComponent, TransformSampleAncestorComponent,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransformPipelineOutput {
    ImplicitTransform,
    OutputRoots(Vec<ComponentId>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformPipelineMergeMode {
    ImplicitPassthrough,
    Explicit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransformPipelineVec3Op {
    Pass,
    Drop,
    TemporalSmooth { smoothing_factor: f32 },
    /// Replace this channel's value with the world translation of an ancestor
    /// TransformComponent. `skip` counts TransformComponent ancestors upward from the
    /// pipeline owner: 0 = the driven T directly above the pipeline, 1 = the next T above
    /// that (e.g. the armature bone above an InputXR splice), etc.
    SampleAncestorTranslation { skip: usize },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransformPipelineQuatOp {
    Pass,
    Drop,
    TemporalFilter { smoothing_factor: f32 },
    /// Replace this channel's value with the world rotation of an ancestor
    /// TransformComponent. Same `skip` semantics as `SampleAncestorTranslation`.
    SampleAncestorRotation { skip: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransformForkTrsStage {
    pub translation_ops: Vec<TransformPipelineVec3Op>,
    pub rotation_ops: Vec<TransformPipelineQuatOp>,
    pub scale_ops: Vec<TransformPipelineVec3Op>,
    pub merge_mode: TransformPipelineMergeMode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransformPipelineStage {
    ForkTrs(TransformForkTrsStage),
    Pipeline(Box<TransformPipeline>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransformPipeline {
    pub owner_component: Option<ComponentId>,
    pub input: TransformPipelineInput,
    pub stages: Vec<TransformPipelineStage>,
    pub output: TransformPipelineOutput,
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
pub struct TransformPipelineSystem {
    last_dt_sec: Option<f32>,
    vec3_temporal_state: HashMap<TransformPipelineStageKey, Vec3TemporalState>,
    quat_temporal_state: HashMap<TransformPipelineStageKey, QuatTemporalState>,
}

impl TransformPipelineSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse_component_tree(
        &self,
        world: &World,
        root: ComponentId,
    ) -> Option<TransformPipeline> {
        if world
            .get_component_by_id_as::<TransformPipelineComponent>(root)
            .is_some()
        {
            return self.parse_pipeline_block(world, root);
        }
        None
    }

    pub fn evaluate_pipeline_node(
        &mut self,
        world: &World,
        root: ComponentId,
        input_world: TransformMatrix,
    ) -> Option<(TransformMatrix, Vec<ComponentId>)> {
        let block = self.parse_component_tree(world, root)?;
        let outputs = match &block.output {
            TransformPipelineOutput::ImplicitTransform => Vec::new(),
            TransformPipelineOutput::OutputRoots(roots) => roots.clone(),
        };
        let world_matrix = self.evaluate_block(&block, input_world, world, self.last_dt_sec);
        Some((world_matrix, outputs))
    }

    pub fn pipeline_for_controller_rotation_smoothing(
        owner_component: Option<ComponentId>,
        smoothing_factor: f32,
    ) -> TransformPipeline {
        TransformPipeline {
            owner_component,
            input: TransformPipelineInput::ParentWorld,
            stages: vec![TransformPipelineStage::ForkTrs(TransformForkTrsStage {
                translation_ops: vec![TransformPipelineVec3Op::Pass],
                rotation_ops: vec![TransformPipelineQuatOp::TemporalFilter { smoothing_factor }],
                scale_ops: vec![TransformPipelineVec3Op::Pass],
                merge_mode: TransformPipelineMergeMode::ImplicitPassthrough,
            })],
            output: TransformPipelineOutput::ImplicitTransform,
        }
    }

    fn parse_pipeline_block(
        &self,
        world: &World,
        root: ComponentId,
    ) -> Option<TransformPipeline> {
        if world
            .get_component_by_id_as::<TransformPipelineComponent>(root)
            .is_none()
        {
            return None;
        }

        let mut stages = Vec::new();
        let mut output_roots = Vec::new();

        for &child in world.children_of(root) {
            if world
                .get_component_by_id_as::<TransformPipelineOutputComponent>(child)
                .is_some()
            {
                output_roots.push(child);
                continue;
            }

            if let Some(stage) = self.parse_stage(world, child) {
                stages.push(stage);
            }
        }

        Some(TransformPipeline {
            owner_component: Some(root),
            input: TransformPipelineInput::ParentWorld,
            stages,
            output: if output_roots.is_empty() {
                TransformPipelineOutput::ImplicitTransform
            } else {
                TransformPipelineOutput::OutputRoots(output_roots)
            },
        })
    }

    fn parse_stage(&self, world: &World, node: ComponentId) -> Option<TransformPipelineStage> {
        if world
            .get_component_by_id_as::<TransformForkTRSComponent>(node)
            .is_some()
        {
            return Some(TransformPipelineStage::ForkTrs(self.parse_fork_trs(world, node)));
        }

        if world
            .get_component_by_id_as::<TransformPipelineComponent>(node)
            .is_some()
        {
            return self
                .parse_pipeline_block(world, node)
                .map(|pipeline| TransformPipelineStage::Pipeline(Box::new(pipeline)));
        }

        None
    }

    fn parse_fork_trs(&self, world: &World, node: ComponentId) -> TransformForkTrsStage {
        let mut translation_ops = vec![TransformPipelineVec3Op::Pass];
        let mut rotation_ops = vec![TransformPipelineQuatOp::Pass];
        let mut scale_ops = vec![TransformPipelineVec3Op::Pass];
        let mut merge_mode = TransformPipelineMergeMode::ImplicitPassthrough;

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
                merge_mode = TransformPipelineMergeMode::Explicit;
            }
        }

        TransformForkTrsStage {
            translation_ops,
            rotation_ops,
            scale_ops,
            merge_mode,
        }
    }

    fn parse_vec3_ops(&self, world: &World, node: ComponentId) -> Vec<TransformPipelineVec3Op> {
        let mut ops = Vec::new();
        for &child in world.children_of(node) {
            if world.get_component_by_id_as::<TransformDropComponent>(child).is_some() {
                ops.push(TransformPipelineVec3Op::Drop);
                continue;
            }
            if let Some(s) = world.get_component_by_id_as::<TransformSampleAncestorComponent>(child) {
                ops.push(TransformPipelineVec3Op::SampleAncestorTranslation { skip: s.skip });
                continue;
            }
            if let Some(filter) = world.get_component_by_id_as::<Vector3TemporalFilterComponent>(child) {
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
            if world.get_component_by_id_as::<TransformDropComponent>(child).is_some() {
                ops.push(TransformPipelineQuatOp::Drop);
                continue;
            }
            if let Some(s) = world.get_component_by_id_as::<TransformSampleAncestorComponent>(child) {
                ops.push(TransformPipelineQuatOp::SampleAncestorRotation { skip: s.skip });
                continue;
            }
            if let Some(filter) = world.get_component_by_id_as::<QuatTemporalFilterComponent>(child) {
                ops.push(TransformPipelineQuatOp::TemporalFilter {
                    smoothing_factor: filter.smoothing_factor,
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
        pipeline: &TransformPipeline,
        input_world: TransformMatrix,
        world: &World,
        dt_sec: Option<f32>,
    ) -> TransformMatrix {
        let mut channels = Self::decompose_matrix(input_world);
        for (stage_index, stage) in pipeline.stages.iter().enumerate() {
            let mut stage_path = vec![stage_index];
            channels = self.evaluate_stage(pipeline.owner_component, stage, channels, &mut stage_path, world, dt_sec);
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
            TransformPipelineStage::Pipeline(pipeline) => {
                self.evaluate_block(pipeline, Self::recompose_matrix(input), world, dt_sec).into()
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

        match fork.merge_mode {
            TransformPipelineMergeMode::ImplicitPassthrough | TransformPipelineMergeMode::Explicit => {
                TransformPipelineChannels {
                    translation,
                    rotation_quat_xyzw,
                    scale,
                }
            }
        }
    }

    /// Walk ancestor TransformComponents from `owner` upward. Returns the world matrix of the
    /// `skip`-th TransformComponent found (0 = first / nearest ancestor, 1 = second, etc.).
    fn sample_ancestor_world(world: &World, owner: ComponentId, skip: usize) -> Option<TransformMatrix> {
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
                TransformPipelineVec3Op::SampleAncestorTranslation { skip } => {
                    owner_component
                        .and_then(|owner| Self::sample_ancestor_world(world, owner, skip))
                        .map(|m| [m[3][0], m[3][1], m[3][2]])
                        .unwrap_or(current)
                }
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
                TransformPipelineQuatOp::SampleAncestorRotation { skip } => {
                    owner_component
                        .and_then(|owner| Self::sample_ancestor_world(world, owner, skip))
                        .map(|m| Self::decompose_matrix(m).rotation_quat_xyzw)
                        .unwrap_or(current)
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
                        let filtered_step_deg =
                            Self::quat_angle_degrees(previous_output, filtered);
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

    fn orthonormalize_basis(
        b0: [f32; 3],
        b1: [f32; 3],
        b2: [f32; 3],
    ) -> [[f32; 3]; 3] {
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
        TransformPipelineSystem::decompose_matrix(value)
    }
}

impl System for TransformPipelineSystem {
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
    use crate::engine::ecs::component::{
        QuatTemporalFilterComponent, TransformForkTRSComponent, TransformMapRotationComponent,
        TransformMapScaleComponent, TransformMapTranslationComponent,
        TransformMergeTRSComponent, TransformPipelineComponent, TransformPipelineOutputComponent,
        Vector3TemporalFilterComponent,
    };
    use crate::engine::ecs::World;

    #[test]
    fn controller_rotation_pipeline_contains_temporal_quat_op() {
        let block = TransformPipelineSystem::pipeline_for_controller_rotation_smoothing(None, 1.0);
        let TransformPipelineStage::ForkTrs(stage) = &block.stages[0] else {
            panic!("expected fork stage");
        };
        assert_eq!(
            stage.rotation_ops,
            vec![TransformPipelineQuatOp::TemporalFilter {
                smoothing_factor: 1.0
            }]
        );
    }

    #[test]
    fn parses_authored_pipeline_component_tree() {
        let mut world = World::default();
        let pipeline = world.add_component(TransformPipelineComponent::new());
        let fork = world.add_component(TransformForkTRSComponent::new());
        let map_translation = world.add_component(TransformMapTranslationComponent::new());
        let vec_filter = world.add_component(
            Vector3TemporalFilterComponent::new().with_smoothing_factor(12.0),
        );
        let map_rotation = world.add_component(TransformMapRotationComponent::new());
        let quat_filter =
            world.add_component(QuatTemporalFilterComponent::new().with_smoothing_factor(18.0));
        let map_scale = world.add_component(TransformMapScaleComponent::new());
        let merge = world.add_component(TransformMergeTRSComponent::new());
        let output = world.add_component(TransformPipelineOutputComponent::new());

        world.set_parent(fork, Some(pipeline)).unwrap();
        world.set_parent(map_translation, Some(fork)).unwrap();
        world.set_parent(vec_filter, Some(map_translation)).unwrap();
        world.set_parent(map_rotation, Some(fork)).unwrap();
        world.set_parent(quat_filter, Some(map_rotation)).unwrap();
        world.set_parent(map_scale, Some(fork)).unwrap();
        world.set_parent(merge, Some(fork)).unwrap();
        world.set_parent(output, Some(pipeline)).unwrap();

        let parser = TransformPipelineSystem::new();
        let block = parser.parse_component_tree(&world, pipeline).expect("pipeline block");
        assert_eq!(block.owner_component, Some(pipeline));
        assert_eq!(block.stages.len(), 1);
        assert_eq!(block.output, TransformPipelineOutput::OutputRoots(vec![output]));
        let TransformPipelineStage::ForkTrs(stage) = &block.stages[0] else {
            panic!("expected fork stage");
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
        assert_eq!(stage.merge_mode, TransformPipelineMergeMode::Explicit);
    }
}
