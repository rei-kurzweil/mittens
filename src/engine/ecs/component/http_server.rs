use super::Component;
use crate::engine::ecs::component::ce_helpers::*;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

#[derive(Debug, Clone, Default)]
pub struct HttpServerComponent {
    pub bind_addr: String,
    pub enabled: bool,
    component: Option<ComponentId>,
}

impl HttpServerComponent {
    pub fn new() -> Self {
        Self {
            bind_addr: String::new(),
            enabled: true,
            component: None,
        }
    }

    pub fn bind(bind_addr: impl Into<String>) -> Self {
        Self::new().with_bind_addr(bind_addr)
    }

    pub fn with_bind_addr(mut self, bind_addr: impl Into<String>) -> Self {
        self.bind_addr = bind_addr.into();
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Component for HttpServerComponent {
    fn name(&self) -> &'static str {
        "http_server"
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
            IntentValue::RegisterHttpServer {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        let mut ce = if self.bind_addr.is_empty() {
            ce("HttpServer")
        } else {
            ce_call("HttpServer", "bind", vec![s(&self.bind_addr)])
        };
        if !self.enabled {
            ce = ce.with_call("enabled", vec![b(false)]);
        }
        ce
    }
}
