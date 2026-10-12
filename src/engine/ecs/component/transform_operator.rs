use super::{Component, ComponentRef, QueryRootMode, resolve_component_ref};
use crate::engine::ecs::{ComponentId, World};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransformVec3Op {
    Pass,
    Drop,
    TemporalSmooth {
        smoothing_factor: f32,
    },
    /// Replace this channel's value with the world translation of an ancestor
    /// TransformComponent. `skip` counts TransformComponent ancestors upward from the
    /// operator owner: 0 = the driven T directly above the operator, 1 = the next T above
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
pub struct TransformForkTrsSettings {
    pub translation_ops: Vec<TransformVec3Op>,
    pub rotation_ops: Vec<TransformQuatOp>,
    pub scale_ops: Vec<TransformVec3Op>,
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

impl Default for TransformForkTrsSettings {
    fn default() -> Self {
        Self {
            translation_ops: vec![TransformVec3Op::Pass],
            rotation_ops: vec![TransformQuatOp::Pass],
            scale_ops: vec![TransformVec3Op::Pass],
        }
    }
}

impl TransformForkTrsSettings {
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

impl TransformForkTrsSettings {
    fn to_mms_ast(&self) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut out = ce_call("TransformOperator", "fork_trs", vec![]);
        for (channel, ops) in [
            ("translation", &self.translation_ops),
            ("scale", &self.scale_ops),
        ] {
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
        for op in &self.rotation_ops {
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

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TransformParentSettings {
    pub target_source: Option<ComponentRef>,
    pub root_source: Option<ComponentRef>,
}

impl TransformParentSettings {
    pub fn resolve_root_component(&self, world: &World) -> Option<ComponentId> {
        let src = self.root_source.as_ref()?;
        resolve_component_ref(world, src, None, QueryRootMode::WorldRoot)
    }

    pub fn resolve_target_component(&self, world: &World) -> Option<ComponentId> {
        let src = self.target_source.as_ref()?;
        let scope_root = match src {
            ComponentRef::Guid(_) => None,
            ComponentRef::Query(_) => self.resolve_root_component(world),
        };
        Self::resolve_component_ref(world, src, scope_root)
    }

    fn resolve_component_ref(
        world: &World,
        src: &ComponentRef,
        scope_root: Option<ComponentId>,
    ) -> Option<ComponentId> {
        match src {
            ComponentRef::Guid(uuid) => world.component_id_by_guid(*uuid),
            ComponentRef::Query(selector) => {
                if let Some(root) = scope_root {
                    return resolve_component_ref(
                        world,
                        &ComponentRef::Query(selector.clone()),
                        Some(root),
                        QueryRootMode::SelfSubtree,
                    );
                }
                resolve_component_ref(
                    world,
                    &ComponentRef::Query(selector.clone()),
                    None,
                    QueryRootMode::WorldRoot,
                )
            }
        }
    }
}

/// Applies the inverse of a referenced transform's authored local matrix to
/// the inherited transform stream. The affected destination is expressed by
/// this component's child topology.
#[derive(Debug, Clone, PartialEq)]
pub struct TransformInverseLocalSettings {
    pub source: ComponentRef,
}

impl TransformInverseLocalSettings {
    pub fn new(source: ComponentRef) -> Self {
        Self { source }
    }

    pub fn resolve_source_component(
        &self,
        world: &World,
        owner: ComponentId,
    ) -> Option<ComponentId> {
        resolve_component_ref(world, &self.source, Some(owner), QueryRootMode::WorldRoot)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformCameraSpecificMode {
    Monoscopic,
    Stereoscopic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransformCameraSettings {
    pub mode: TransformCameraSpecificMode,
}

/// A set of world translation axes. It never reorders coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranslationAxes(u8);
impl TranslationAxes {
    pub const XYZ: Self = Self(7);
    pub const Y: Self = Self(2);
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut bits = 0;
        for ch in text.chars() {
            let bit = match ch {
                'x' => 1,
                'y' => 2,
                'z' => 4,
                _ => {
                    return Err(format!(
                        "invalid translation axis {ch:?}; expected a nonempty subset of xyz"
                    ));
                }
            };
            if bits & bit != 0 {
                return Err(format!("duplicate translation axis {ch:?}"));
            }
            bits |= bit;
        }
        if bits == 0 {
            return Err("translation axes must not be empty".into());
        }
        Ok(Self(bits))
    }
    pub fn contains(self, axis: usize) -> bool {
        self.0 & (1 << axis) != 0
    }
    pub fn as_str(self) -> &'static str {
        match self.0 {
            1 => "x",
            2 => "y",
            3 => "xy",
            4 => "z",
            5 => "xz",
            6 => "yz",
            _ => "xyz",
        }
    }
}
impl Default for TranslationAxes {
    fn default() -> Self {
        Self::XYZ
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TransformSampleTranslationSettings {
    pub source: Option<ComponentRef>,
    pub root_source: Option<ComponentRef>,
    pub axes: TranslationAxes,
}

/// Typed configuration for one live transform operator. Referenced sources are
/// data dependencies; ordinary children inherit the evaluated output basis.
#[derive(Debug, Clone, PartialEq)]
pub enum TransformOperatorKind {
    ForkTrs(TransformForkTrsSettings),
    Parent(TransformParentSettings),
    SampleTranslation(TransformSampleTranslationSettings),
    InverseLocal(TransformInverseLocalSettings),
    /// A camera settings marker configures its parent anchor T, and contains a
    /// settings T. This specialized role is separate from ordinary output nesting.
    CameraSpecific(TransformCameraSettings),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransformOperatorComponent {
    pub kind: TransformOperatorKind,
}

impl TransformOperatorComponent {
    pub fn fork_trs() -> Self {
        Self {
            kind: TransformOperatorKind::ForkTrs(TransformForkTrsSettings::default()),
        }
    }
    pub fn parent() -> Self {
        Self {
            kind: TransformOperatorKind::Parent(TransformParentSettings::default()),
        }
    }
    pub fn inverse_local(source: ComponentRef) -> Self {
        Self {
            kind: TransformOperatorKind::InverseLocal(TransformInverseLocalSettings::new(source)),
        }
    }
    pub fn sample_translation(source: ComponentRef) -> Self {
        Self {
            kind: TransformOperatorKind::SampleTranslation(TransformSampleTranslationSettings {
                source: Some(source),
                ..Default::default()
            }),
        }
    }
    pub fn sample_translation_settings(&self) -> Option<&TransformSampleTranslationSettings> {
        if let TransformOperatorKind::SampleTranslation(settings) = &self.kind {
            Some(settings)
        } else {
            None
        }
    }
    pub fn sample_translation_settings_mut(
        &mut self,
    ) -> Option<&mut TransformSampleTranslationSettings> {
        if let TransformOperatorKind::SampleTranslation(settings) = &mut self.kind {
            Some(settings)
        } else {
            None
        }
    }
    pub fn with_axes(mut self, axes: TranslationAxes) -> Self {
        self.sample_translation_settings_mut()
            .expect("axes requires sample_translation")
            .axes = axes;
        self
    }
    pub fn camera_specific(mode: TransformCameraSpecificMode) -> Self {
        Self {
            kind: TransformOperatorKind::CameraSpecific(TransformCameraSettings { mode }),
        }
    }

    pub fn fork_trs_settings(&self) -> Option<&TransformForkTrsSettings> {
        if let TransformOperatorKind::ForkTrs(settings) = &self.kind {
            Some(settings)
        } else {
            None
        }
    }
    pub fn fork_trs_settings_mut(&mut self) -> Option<&mut TransformForkTrsSettings> {
        if let TransformOperatorKind::ForkTrs(settings) = &mut self.kind {
            Some(settings)
        } else {
            None
        }
    }
    pub fn parent_settings(&self) -> Option<&TransformParentSettings> {
        if let TransformOperatorKind::Parent(settings) = &self.kind {
            Some(settings)
        } else {
            None
        }
    }
    pub fn parent_settings_mut(&mut self) -> Option<&mut TransformParentSettings> {
        if let TransformOperatorKind::Parent(settings) = &mut self.kind {
            Some(settings)
        } else {
            None
        }
    }
    pub fn inverse_local_settings(&self) -> Option<&TransformInverseLocalSettings> {
        if let TransformOperatorKind::InverseLocal(settings) = &self.kind {
            Some(settings)
        } else {
            None
        }
    }
    pub fn inverse_local_settings_mut(&mut self) -> Option<&mut TransformInverseLocalSettings> {
        if let TransformOperatorKind::InverseLocal(settings) = &mut self.kind {
            Some(settings)
        } else {
            None
        }
    }
    pub fn camera_settings(&self) -> Option<&TransformCameraSettings> {
        if let TransformOperatorKind::CameraSpecific(settings) = &self.kind {
            Some(settings)
        } else {
            None
        }
    }

    /// Rust builders require the matching constructor; MMS validates the selected
    /// kind and returns an authoring error before modifying its settings.
    pub fn with_drop(mut self, channel: TransformChannel) -> Self {
        self.set_channel(channel, true);
        self
    }
    pub fn set_channel(&mut self, channel: TransformChannel, drop: bool) {
        self.fork_trs_settings_mut()
            .expect("channel configuration requires fork_trs")
            .set_channel(channel, drop);
    }
    pub fn push_vec3(
        &mut self,
        channel: TransformChannel,
        op: TransformVec3Op,
    ) -> Result<(), String> {
        self.fork_trs_settings_mut()
            .ok_or("channel configuration requires fork_trs")?
            .push_vec3(channel, op)
    }
    pub fn push_rotation(&mut self, op: TransformQuatOp) {
        self.fork_trs_settings_mut()
            .expect("rotation configuration requires fork_trs")
            .push_rotation(op);
    }
    pub fn with_source(mut self, source: ComponentRef) -> Self {
        match &mut self.kind {
            TransformOperatorKind::Parent(settings) => settings.target_source = Some(source),
            TransformOperatorKind::InverseLocal(settings) => settings.source = source,
            TransformOperatorKind::SampleTranslation(settings) => settings.source = Some(source),
            _ => {
                panic!("source configuration requires parent, inverse_local, or sample_translation")
            }
        }
        self
    }
    pub fn with_root_source(mut self, source: ComponentRef) -> Self {
        match &mut self.kind {
            TransformOperatorKind::Parent(settings) => settings.root_source = Some(source),
            TransformOperatorKind::SampleTranslation(settings) => {
                settings.root_source = Some(source)
            }
            _ => panic!("root configuration requires parent or sample_translation"),
        }
        self
    }
}

impl Component for TransformOperatorComponent {
    fn name(&self) -> &'static str {
        "transform_operator"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        if matches!(
            self.kind,
            TransformOperatorKind::Parent(_)
                | TransformOperatorKind::InverseLocal(_)
                | TransformOperatorKind::SampleTranslation(_)
        ) {
            emit.push_intent_now(
                component,
                crate::engine::ecs::IntentValue::UpdateTransformWorld {
                    component_id: component,
                },
            );
        }
    }
    fn to_mms_ast(&self, _world: &World) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        fn source_expr(source: &ComponentRef) -> crate::scripting::ast::Expression {
            match source {
                ComponentRef::Guid(guid) => s(&format!("@uuid:{guid}")),
                ComponentRef::Query(query) => s(query),
            }
        }
        match &self.kind {
            TransformOperatorKind::ForkTrs(settings) => settings.to_mms_ast(),
            TransformOperatorKind::SampleTranslation(settings) => {
                let mut out = ce_call("TransformOperator", "sample_translation", vec![]);
                if let Some(source) = &settings.source {
                    out = out.with_call("source", vec![source_expr(source)]);
                }
                if let Some(root) = &settings.root_source {
                    out = out.with_call("root", vec![source_expr(root)]);
                }
                out.with_call("axes", vec![s(settings.axes.as_str())])
            }
            TransformOperatorKind::Parent(settings) => {
                let mut out = ce_call(
                    "TransformOperator",
                    "parent",
                    settings.target_source.iter().map(source_expr).collect(),
                );
                if let Some(root) = &settings.root_source {
                    out = out.with_call("root", vec![source_expr(root)]);
                }
                out
            }
            TransformOperatorKind::InverseLocal(settings) => ce_call(
                "TransformOperator",
                "inverse_local",
                vec![source_expr(&settings.source)],
            ),
            TransformOperatorKind::CameraSpecific(settings) => ce_call(
                "TransformOperator",
                "camera_specific",
                vec![s(match settings.mode {
                    TransformCameraSpecificMode::Monoscopic => "monoscopic",
                    TransformCameraSpecificMode::Stereoscopic => "stereoscopic",
                })],
            ),
        }
    }
}
