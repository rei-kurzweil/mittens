use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use crate::engine::graphics::bounds::Aabb;

/// Layout-owned local boxes for a styled layout item.
///
/// The styled item's `TransformComponent` is placed at the content-box origin,
/// so both boxes are expressed in that local coordinate space.
#[derive(Debug, Clone, Copy)]
pub struct LayoutBoundsComponent {
    pub content_local: Aabb,
    pub padding_local: Aabb,
}

impl LayoutBoundsComponent {
    pub fn new(content_local: Aabb, padding_local: Aabb) -> Self {
        Self {
            content_local,
            padding_local,
        }
    }
}

impl Component for LayoutBoundsComponent {
    fn name(&self) -> &'static str {
        "layout_bounds"
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
        use crate::engine::ecs::component::ce_helpers::{CeBuilder, array, ce_call, num};

        let content = vec![
            array(
                self.content_local
                    .min
                    .iter()
                    .map(|&value| num(value as f64))
                    .collect(),
            ),
            array(
                self.content_local
                    .max
                    .iter()
                    .map(|&value| num(value as f64))
                    .collect(),
            ),
        ];
        let padding = vec![
            array(
                self.padding_local
                    .min
                    .iter()
                    .map(|&value| num(value as f64))
                    .collect(),
            ),
            array(
                self.padding_local
                    .max
                    .iter()
                    .map(|&value| num(value as f64))
                    .collect(),
            ),
        ];

        ce_call("LayoutBounds", "content_box", content).with_call("padding_box", padding)
    }
}
