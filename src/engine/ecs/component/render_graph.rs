use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy)]
pub struct RenderGraphComponent {
    pub enabled: bool,
}

impl Default for RenderGraphComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderGraphComponent {
    pub fn new() -> Self {
        Self { enabled: true }
    }

    pub fn on() -> Self {
        Self { enabled: true }
    }

    pub fn off() -> Self {
        Self { enabled: false }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Component for RenderGraphComponent {
    fn name(&self) -> &'static str {
        "render_graph"
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
            crate::engine::ecs::IntentValue::RegisterRenderGraph {
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
        ce_call("RenderGraph", ctor, vec![])
    }
}
