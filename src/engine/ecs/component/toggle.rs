use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, component::Component};

/// Independent boolean UI control. The styled owner is the node carrying this component,
/// or its parent when `Toggle` is authored as a sidecar child.
#[derive(Debug, Clone, Copy)]
pub struct ToggleComponent {
    value: bool,
    component: Option<ComponentId>,
}

impl ToggleComponent {
    pub fn new(value: bool) -> Self {
        Self {
            value,
            component: None,
        }
    }
    pub fn on() -> Self {
        Self::new(true)
    }
    pub fn off() -> Self {
        Self::new(false)
    }
    pub fn value(&self) -> bool {
        self.value
    }
    pub(crate) fn set_value(&mut self, value: bool) {
        self.value = value;
    }
}

impl Default for ToggleComponent {
    fn default() -> Self {
        Self::off()
    }
}

impl Component for ToggleComponent {
    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
    }
    fn name(&self) -> &'static str {
        "toggle"
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
            IntentValue::ToggleSet {
                component_ids: vec![component],
                value: self.value,
            },
        );
    }
    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        crate::engine::ecs::component::ce_helpers::ce_call(
            "Toggle",
            if self.value { "on" } else { "off" },
            vec![],
        )
    }
}
