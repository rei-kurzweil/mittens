use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Legacy parameter transport retained for authoring compatibility and native Grid.
///
/// Legacy compatibility input: Toon lighting now uses a fixed three bands.
/// Native Grid still transports signed visual spacing through this component.
/// Intended to be attached as a descendant of a `RenderableComponent`.
#[derive(Debug, Clone, Copy)]
pub struct LightQuantizationComponent {
    pub quant_steps: f32,
}

impl LightQuantizationComponent {
    /// Default toon quantization steps.
    pub const DEFAULT_STEPS: f32 = 3.0;

    pub fn new() -> Self {
        Self {
            quant_steps: Self::DEFAULT_STEPS,
        }
    }

    pub fn steps(steps: f32) -> Self {
        Self { quant_steps: steps }
    }

    pub fn with_steps(mut self, steps: f32) -> Self {
        self.quant_steps = steps;
        self
    }
}

impl Default for LightQuantizationComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for LightQuantizationComponent {
    fn name(&self) -> &'static str {
        "light_quantization"
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
            crate::engine::ecs::IntentValue::RegisterLightQuantization {
                component_id: component,
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call(
            "LightQuantization",
            "steps",
            vec![num(self.quant_steps as f64)],
        )
    }
}
