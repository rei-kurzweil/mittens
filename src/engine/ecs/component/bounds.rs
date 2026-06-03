use crate::engine::ecs::component::Component;
use crate::engine::ecs::ComponentId;
use crate::engine::graphics::bounds::Aabb;

/// Cached local-space AABB for a sibling `RenderableComponent`.
///
/// Attached automatically as a child of a renderable during
/// `SystemWorld::register_renderable` whenever the renderable's mesh has a
/// known local AABB (see `graphics::bounds::mesh_local_aabb`). Layout reads
/// this to size containers around renderable children without having to mutate
/// the renderable's own transform.
pub struct BoundsComponent {
    pub local: Aabb,
}

impl BoundsComponent {
    pub fn new(local: Aabb) -> Self {
        Self { local }
    }
}

impl Component for BoundsComponent {
    fn name(&self) -> &'static str {
        "bounds"
    }

    fn set_id(&mut self, _component: ComponentId) {}

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
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call(
            "Bounds",
            "aabb",
            vec![
                array(nums(self.local.min.iter().map(|&v| v as f64))),
                array(nums(self.local.max.iter().map(|&v| v as f64))),
            ],
        )
    }
}
