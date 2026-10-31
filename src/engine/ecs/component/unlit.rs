use crate::engine::ecs::component::Component;
use crate::engine::ecs::component::ce_helpers::ce;

/// Selects a non-emissive, lighting-independent material for standard
/// renderables in the owning scope. Nearer material declarations can override
/// it. Color, opacity and texture inputs remain active; lighting and bloom do not.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnlitComponent;

impl Component for UnlitComponent {
    fn name(&self) -> &'static str {
        "unlit"
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
        ce("Unlit")
    }
}
