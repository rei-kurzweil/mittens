use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Default)]
pub struct OpenXRComponent {
    pub enabled: bool,
}

impl OpenXRComponent {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn on() -> Self {
        Self { enabled: true }
    }

    pub fn off() -> Self {
        Self { enabled: false }
    }
}

impl Component for OpenXRComponent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "openxr"
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterOpenxr {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(&self, _world: &crate::engine::ecs::World) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = if self.enabled { "on" } else { "off" };
        ce_call("OpenXR", ctor, vec![])
    }
}
