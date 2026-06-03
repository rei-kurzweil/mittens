use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Default)]
pub struct EmissivePassComponent;

impl EmissivePassComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for EmissivePassComponent {
    fn name(&self) -> &'static str {
        "emissive_pass"
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
    ) -> crate::meow_meow::ast::ComponentExpression {
        crate::engine::ecs::component::ce_helpers::ce("EmissivePass")
    }
}
