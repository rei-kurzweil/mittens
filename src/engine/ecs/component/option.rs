use crate::engine::ecs::{ComponentId, component::Component};

#[derive(Debug, Clone, Default)]
pub struct OptionComponent {
    component: Option<ComponentId>,
}

impl OptionComponent {
    pub fn new() -> Self {
        Self { component: None }
    }
}

impl Component for OptionComponent {
    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
    }

    fn name(&self) -> &'static str {
        "option"
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
        ce_call("Option", "", vec![])
    }
}
