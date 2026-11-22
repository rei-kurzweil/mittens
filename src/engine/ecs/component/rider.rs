use super::{Component, ComponentRef};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// Declares a participant whose movement root can be attached to a Mountable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiderComponent {
    pub anchor: Option<ComponentRef>,
    pub movement_root: Option<ComponentRef>,
    pub input: Option<ComponentRef>,
    pub enabled: bool,
}

impl Default for RiderComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl RiderComponent {
    pub fn new() -> Self {
        Self {
            anchor: None,
            movement_root: None,
            input: None,
            enabled: true,
        }
    }

    pub fn anchor(mut self, value: ComponentRef) -> Self {
        self.anchor = Some(value);
        self
    }

    pub fn movement_root(mut self, value: ComponentRef) -> Self {
        self.movement_root = Some(value);
        self
    }

    pub fn input(mut self, value: ComponentRef) -> Self {
        self.input = Some(value);
        self
    }

    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }
}

impl Component for RiderComponent {
    fn name(&self) -> &'static str {
        "rider"
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
            IntentValue::RegisterRider {
                component_id: component,
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use super::ce_helpers::*;
        use crate::scripting::ast::Expression;

        fn reference(value: &ComponentRef) -> Expression {
            match value {
                ComponentRef::Guid(guid) => s(&format!("@uuid:{guid}")),
                ComponentRef::Query(query) => s(query),
            }
        }

        let mut expression = ce("Rider");
        if let Some(value) = &self.anchor {
            expression = expression.with_call("anchor", vec![reference(value)]);
        }
        if let Some(value) = &self.movement_root {
            expression = expression.with_call("movement_root", vec![reference(value)]);
        }
        if let Some(value) = &self.input {
            expression = expression.with_call("input", vec![reference(value)]);
        }
        if !self.enabled {
            expression = expression.with_call("enabled", vec![b(false)]);
        }
        expression
    }
}
