use super::Component;
use crate::engine::ecs::ComponentId;

/// Controls the global beat clock tempo (BPM).
///
/// Intended to be singleton-like: the most recently registered ClockComponent wins.
#[derive(Debug, Clone, Copy)]
pub struct ClockComponent {
    pub bpm: f64,

    component: Option<ComponentId>,
}

impl ClockComponent {
    pub fn new() -> Self {
        Self {
            bpm: 120.0,
            component: None,
        }
    }

    pub fn with_bpm(mut self, bpm: f64) -> Self {
        self.bpm = bpm;
        self
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }
}

impl Default for ClockComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ClockComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "clock"
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterClock {
                component_ids: vec![component],
            },
        );
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
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call("Clock", "bpm", vec![num(self.bpm)])
    }
}
