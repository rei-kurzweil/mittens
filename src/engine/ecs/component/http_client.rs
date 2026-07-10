use super::Component;
use crate::engine::ecs::component::ce_helpers::*;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

#[derive(Debug, Clone, Default)]
pub struct HttpClientComponent {
    pub enabled: bool,
    pub timeout_ms: Option<u64>,
    component: Option<ComponentId>,
}

impl HttpClientComponent {
    pub fn new() -> Self {
        Self {
            enabled: true,
            timeout_ms: None,
            component: None,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }
}

impl Component for HttpClientComponent {
    fn name(&self) -> &'static str {
        "http_client"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
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
            IntentValue::RegisterHttpClient {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        let mut ce = ce("HttpClient");
        if !self.enabled {
            ce = ce.with_call("enabled", vec![b(false)]);
        }
        if let Some(timeout_ms) = self.timeout_ms {
            ce = ce.with_call("timeout_ms", vec![num(timeout_ms as f64)]);
        }
        ce
    }
}
