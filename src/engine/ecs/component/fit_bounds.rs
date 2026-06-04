use super::Component;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitBoundsMode {
    RenderableOnly,
    LayoutAware,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitBoundsTarget {
    ExplicitBounds,
    ParentPaddingBox,
}

#[derive(Debug, Clone, Copy)]
pub struct FitBoundsComponent {
    pub mode: FitBoundsMode,
    pub target: FitBoundsTarget,
    pub target_bounds: [f32; 6],
}

impl FitBoundsComponent {
    pub fn new() -> Self {
        Self {
            mode: FitBoundsMode::RenderableOnly,
            target: FitBoundsTarget::ExplicitBounds,
            target_bounds: [-0.5, -0.5, -0.5, 0.5, 0.5, 0.5],
        }
    }
}

impl Default for FitBoundsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for FitBoundsComponent {
    fn name(&self) -> &'static str {
        "fit_bounds"
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
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::{array, ce_call, num, CeBuilder};

        let mut expr = match self.mode {
            FitBoundsMode::RenderableOnly => ce_call("FitBounds", "renderable_only", vec![]),
            FitBoundsMode::LayoutAware => ce_call("FitBounds", "layout_aware", vec![]),
        };

        expr = match self.target {
            FitBoundsTarget::ExplicitBounds => expr.with_call(
                "to",
                vec![array(
                    self.target_bounds
                        .iter()
                        .map(|&value| num(value as f64))
                        .collect(),
                )],
            ),
            FitBoundsTarget::ParentPaddingBox => expr.with_call("to_container", vec![]),
        };

        expr
    }
}
