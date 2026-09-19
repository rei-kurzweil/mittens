use crate::engine::ecs::component::Component;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// Inverted-hull outline applied to a renderable or inherited by descendant renderables.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToonOutlineComponent {
    pub color: [f32; 4],
    /// Hull expansion in world-space engine units.
    pub width: f32,
    source_component: Option<ComponentId>,
}

impl ToonOutlineComponent {
    pub const DEFAULT_COLOR: [f32; 4] = [0.02, 0.02, 0.03, 1.0];
    pub const DEFAULT_WIDTH: f32 = 0.01;

    pub fn new() -> Self {
        Self {
            color: Self::DEFAULT_COLOR,
            width: Self::DEFAULT_WIDTH,
            source_component: None,
        }
    }

    pub fn with_width(mut self, width: f32) -> Self {
        self.width = if width.is_finite() {
            width.max(0.0)
        } else {
            Self::DEFAULT_WIDTH
        };
        self
    }

    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        if color.iter().all(|value| value.is_finite()) {
            self.color = color.map(|value| value.clamp(0.0, 1.0));
        }
        self
    }

    pub(crate) fn projected_from(mut self, source_component: ComponentId) -> Self {
        self.source_component = Some(source_component);
        self
    }

    pub(crate) fn source_component(self) -> Option<ComponentId> {
        self.source_component
    }

    pub(crate) fn gpu_params(self) -> crate::engine::graphics::visual_world::ToonOutlineParams {
        crate::engine::graphics::visual_world::ToonOutlineParams {
            color: self.color,
            width: self.width,
        }
    }
}

impl Default for ToonOutlineComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ToonOutlineComponent {
    fn name(&self) -> &'static str {
        "toon_outline"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            IntentValue::RegisterToonOutline {
                component_id: component,
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;

        let mut expression = ce("ToonOutline");
        if self.width.to_bits() != Self::DEFAULT_WIDTH.to_bits() {
            expression = expression.with_call("width", vec![num(self.width as f64)]);
        }
        if self.color.map(f32::to_bits) != Self::DEFAULT_COLOR.map(f32::to_bits) {
            expression = expression.with_call(
                "color",
                vec![array(self.color.map(|value| num(value as f64)).to_vec())],
            );
        }
        expression
    }
}
