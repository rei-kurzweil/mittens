use super::Component;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

#[derive(Debug, Clone, Default)]
pub struct RouterComponent {
    pub target_name: Option<String>,
    pub ignore_names: Vec<String>,
    component: Option<ComponentId>,
}

impl RouterComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_target_name(mut self, target_name: impl Into<String>) -> Self {
        self.target_name = Some(target_name.into());
        self
    }

    pub fn with_ignored_names<I, S>(mut self, ignore_names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.ignore_names = ignore_names.into_iter().map(Into::into).collect();
        self
    }
}

impl Component for RouterComponent {
    fn name(&self) -> &'static str {
        "router"
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
            IntentValue::RegisterRouter {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(&self, _world: &crate::engine::ecs::World) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut ce = ce("Router");
        if let Some(target) = &self.target_name {
            ce = ce.with_call("target", vec![s(target)]);
        }
        if !self.ignore_names.is_empty() {
            let items: Vec<_> = self.ignore_names.iter().map(|n| s(n)).collect();
            ce = ce.with_call("ignore", vec![array(items)]);
        }
        ce
    }
}