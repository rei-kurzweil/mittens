use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone)]
pub struct XrComponent {
    pub enabled: bool,
}

impl Default for XrComponent {
    fn default() -> Self {
        Self::on()
    }
}

impl XrComponent {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn on() -> Self {
        Self { enabled: true }
    }

    pub fn off() -> Self {
        Self { enabled: false }
    }

    pub fn openxr() -> Self {
        Self::on()
    }
}

impl Component for XrComponent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "xr"
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterXr {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = if self.enabled { "on" } else { "off" };
        ce_call("XR", ctor, vec![])
    }
}
