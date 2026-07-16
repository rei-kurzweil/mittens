use super::Component;

#[derive(Debug, Clone, Copy, Default)]
pub struct TransformForkTRSComponent;

impl TransformForkTRSComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for TransformForkTRSComponent {
    fn name(&self) -> &'static str {
        "transform_fork_trs"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
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
            "TransformSampleAncestor",
            "skip",
            vec![num(self.skip as f64)],
        )
    }
}
