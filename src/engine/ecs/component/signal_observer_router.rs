use crate::engine::ecs::component::Component;
use crate::engine::ecs::component::ce_helpers::*;
use crate::engine::ecs::{ComponentId, SignalEmitter};

/// Signal routing operator: filters signal observers (handlers) on this node.
#[derive(Debug, Clone, Default)]
pub struct SignalObserverRouterComponent {
    /// Names of handlers that are forbidden from receiving signals on this node.
    pub blacklist: Vec<String>,

    /// (Optional) If non-empty, only allow handlers present in this list.
    pub whitelist: Vec<String>,
}

impl SignalObserverRouterComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_blacklist(mut self, blacklist: Vec<String>) -> Self {
        self.blacklist = blacklist;
        self
    }

    pub fn with_whitelist(mut self, whitelist: Vec<String>) -> Self {
        self.whitelist = whitelist;
        self
    }
}

impl Component for SignalObserverRouterComponent {
    fn name(&self) -> &'static str {
        "signal_observer_router"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, _emit: &mut dyn SignalEmitter, _component: ComponentId) {}

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        let mut ce = ce("ObserverRouter");
        if !self.blacklist.is_empty() {
            ce = ce.with_call("blacklist", vec![ls(&self.blacklist)]);
        }
        if !self.whitelist.is_empty() {
            ce = ce.with_call("whitelist", vec![ls(&self.whitelist)]);
        }
        ce
    }
}
