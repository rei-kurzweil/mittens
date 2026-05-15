use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy)]
pub struct EmissiveComponent {
    pub intensity: f32,
}

impl Default for EmissiveComponent {
    fn default() -> Self {
        Self::on()
    }
}

impl EmissiveComponent {
    pub fn new(intensity: f32) -> Self {
        Self {
            intensity: if intensity.is_finite() {
                intensity.max(0.0)
            } else {
                0.0
            },
        }
    }

    pub fn on() -> Self {
        Self { intensity: 1.0 }
    }

    pub fn off() -> Self {
        Self { intensity: 0.0 }
    }
}

impl Component for EmissiveComponent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "emissive"
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterEmissive {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(&self) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = if self.intensity == 0.0 { "off" } else { "on" };
        let mut ce = ce_call("Emissive", ctor, vec![]);
        if self.intensity != 0.0 && self.intensity != 1.0 {
            ce = ce.with_call("intensity", vec![num(self.intensity as f64)]);
        }
        ce
    }
}
