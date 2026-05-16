use super::Component;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// Pipeline operator: route matching intents upward to the first ancestor whose type matches.
#[derive(Debug, Clone, Default)]
pub struct SignalRouteUpwardComponent {
    pub intent_kind: String,
    pub parent_type: String,
}

impl SignalRouteUpwardComponent {
    pub fn new(intent_kind: impl Into<String>, parent_type: impl Into<String>) -> Self {
        Self {
            intent_kind: intent_kind.into(),
            parent_type: parent_type.into(),
        }
    }
}

impl Component for SignalRouteUpwardComponent {
    fn name(&self) -> &'static str {
        "signal_route_upward"
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
            IntentValue::RegisterSignalRouteUpward {
                component_ids: vec![component],
            },
        );
    }

    fn cleanup(&mut self, emit: &mut dyn SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            IntentValue::RemoveSignalRouteUpward {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(&self, _world: &crate::engine::ecs::World) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call(
            "SignalRouteUpward",
            "new",
            vec![s(&self.intent_kind), s(&self.parent_type)],
        )
    }
}
