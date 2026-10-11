use super::Component;
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransformVec3Op {
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
pub enum TransformQuatOp {
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
pub struct TransformForkTRSComponent {
    pub translation_ops: Vec<TransformVec3Op>,
    pub rotation_ops: Vec<TransformQuatOp>,
    pub scale_ops: Vec<TransformVec3Op>,
    component_id: Option<ComponentId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransformChannel {
    Translation,
    Rotation,
    Scale,
}

impl TransformChannel {
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "translation" => Ok(Self::Translation),
            "rotation" => Ok(Self::Rotation),
            "scale" => Ok(Self::Scale),
            _ => Err(format!(
                "unknown transform channel {name:?}; expected translation, rotation, or scale"
            )),
        }
    }
}

impl Default for TransformForkTRSComponent {
    fn default() -> Self {
        Self {
            translation_ops: vec![TransformVec3Op::Pass],
            rotation_ops: vec![TransformQuatOp::Pass],
            scale_ops: vec![TransformVec3Op::Pass],
            component_id: None,
        }
    }
}

impl TransformForkTRSComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_drop(mut self, channel: TransformChannel) -> Self {
        self.set_channel(channel, true);
        self
    }

    pub fn set_channel(&mut self, channel: TransformChannel, drop: bool) {
        match channel {
            TransformChannel::Rotation => {
                self.rotation_ops = vec![if drop {
                    TransformQuatOp::Drop
                } else {
                    TransformQuatOp::Pass
                }]
            }
            TransformChannel::Translation => {
                self.translation_ops = vec![if drop {
                    TransformVec3Op::Drop
                } else {
                    TransformVec3Op::Pass
                }]
            }
            TransformChannel::Scale => {
                self.scale_ops = vec![if drop {
                    TransformVec3Op::Drop
                } else {
                    TransformVec3Op::Pass
                }]
            }
        }
    }

    pub fn push_vec3(
        &mut self,
        channel: TransformChannel,
        op: TransformVec3Op,
    ) -> Result<(), String> {
        let ops = match channel {
            TransformChannel::Translation => &mut self.translation_ops,
            TransformChannel::Scale => &mut self.scale_ops,
            TransformChannel::Rotation => {
                return Err("vec3 operation cannot configure rotation".into());
            }
        };
        if ops.as_slice() == [TransformVec3Op::Pass] {
            ops.clear();
        }
        ops.push(op);
        Ok(())
    }

    pub fn push_rotation(&mut self, op: TransformQuatOp) {
        if self.rotation_ops.as_slice() == [TransformQuatOp::Pass] {
            self.rotation_ops.clear();
        }
        self.rotation_ops.push(op);
    }
}

impl Component for TransformForkTRSComponent {
    fn name(&self) -> &'static str {
        "transform_fork_trs"
    }
    fn set_id(&mut self, id: ComponentId) {
        self.component_id = Some(id);
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn to_mms_ast(
        &self,
        world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        // Legacy Rust-built maps are folded into values, never emitted as output children.
        let parsed = self.component_id.and_then(|id| {
            crate::engine::ecs::system::TransformStreamSystem::new().parse_component_tree(world, id)
        });
        let (translation, rotation, scale) = match parsed.as_ref().and_then(|p| p.stages.first()) {
            Some(crate::engine::ecs::system::transform_stream_system::TransformPipelineStage::ForkTrs(stage)) => (&stage.translation_ops, &stage.rotation_ops, &stage.scale_ops),
            None => (&self.translation_ops, &self.rotation_ops, &self.scale_ops),
        };
        let mut out = ce_call("TransformOperator", "fork_trs", vec![]);
        for (channel, ops) in [("translation", translation), ("scale", scale)] {
            for op in ops {
                out = match *op {
                    TransformVec3Op::Pass => out,
                    TransformVec3Op::Drop => out.with_call("drop", vec![s(channel)]),
                    TransformVec3Op::TemporalSmooth { smoothing_factor } => {
                        out.with_call("smooth", vec![s(channel), num(smoothing_factor as f64)])
                    }
                    TransformVec3Op::SampleAncestorTranslation { skip } => {
                        out.with_call("sample_ancestor", vec![s(channel), num(skip as f64)])
                    }
                };
            }
        }
        for op in rotation {
            out = match *op {
                TransformQuatOp::Pass => out,
                TransformQuatOp::Drop => out.with_call("drop", vec![s("rotation")]),
                TransformQuatOp::TemporalFilter { smoothing_factor } => {
                    out.with_call("smooth", vec![s("rotation"), num(smoothing_factor as f64)])
                }
                TransformQuatOp::SampleAncestorRotation { skip } => {
                    out.with_call("sample_ancestor", vec![s("rotation"), num(skip as f64)])
                }
                TransformQuatOp::ExtractYaw => out.with_call("extract_yaw", vec![]),
                TransformQuatOp::YawFollow {
                    threshold,
                    rate,
                    initial_yaw,
                    forward_plus_z,
                } => out
                    .with_call("yaw_follow", vec![num(threshold as f64), num(rate as f64)])
                    .with_call("forward_plus_z", vec![b(forward_plus_z)])
                    .with_call("initial_yaw", vec![num(initial_yaw as f64)]),
            };
        }
        out
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TransformMergeTRSComponent;

impl TransformMergeTRSComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for TransformMergeTRSComponent {
    fn name(&self) -> &'static str {
        "transform_merge_trs"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TransformDropComponent;

impl TransformDropComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for TransformDropComponent {
    fn name(&self) -> &'static str {
        "transform_drop"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Samples the world transform of an ancestor TransformComponent and injects it into the
/// pipeline channel this component is placed under (TransformMapTranslation or
/// TransformMapRotation).
///
/// `skip` controls how many TransformComponent ancestors to climb past from the pipeline
/// owner before sampling. The walk starts at the pipeline component and goes up:
///
/// - `skip = 0` — the driven TransformComponent directly above the pipeline (same as Pass)
/// - `skip = 1` — the next TransformComponent above that (e.g. the armature bone above the
///   InputXR-driven T in a splice topology)
///
/// The default is `skip = 1`, which is the common case for head/neck bone rotation splices.
#[derive(Debug, Clone, Copy)]
pub struct TransformSampleAncestorComponent {
    pub skip: usize,
}

impl TransformSampleAncestorComponent {
    pub fn new() -> Self {
        Self { skip: 1 }
    }

    pub fn with_skip(mut self, skip: usize) -> Self {
        self.skip = skip;
        self
    }
}

impl Default for TransformSampleAncestorComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for TransformSampleAncestorComponent {
    fn name(&self) -> &'static str {
        "transform_sample_ancestor"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call(
            "TransformOperator",
            "sample_ancestor",
            vec![s("translation"), num(self.skip as f64)],
        )
    }
}
