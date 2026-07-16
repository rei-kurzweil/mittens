use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Per-instance color for a renderable.
///
/// Intended to be attached as a descendant of a `RenderableComponent`.
#[derive(Debug, Clone, Copy)]
pub struct ColorComponent {
    pub rgba: [f32; 4],
}

impl ColorComponent {
    pub fn new() -> Self {
        Self {
            rgba: [1.0, 1.0, 1.0, 1.0],
        }
    }

    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { rgba: [r, g, b, a] }
    }

    pub fn with_rgba(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.rgba = [r, g, b, a];
        self
    }
}

impl Default for ColorComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ColorComponent {
    fn name(&self) -> &'static str {
        "color"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterColor {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call("Color", "rgba", nums(self.rgba.iter().map(|&v| v as f64)))
    }
}
