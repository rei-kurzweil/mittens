use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

use crate::engine::ecs::system::model::collision_types::CollisionShape;

/// Explicit collision shape definition.
///
/// Intended to be added as a child of a `CollisionComponent`.
#[derive(Debug, Clone)]
pub struct CollisionShapeComponent {
    pub shape: CollisionShape,

    component: Option<ComponentId>,
}

impl CollisionShapeComponent {
    pub fn new(shape: CollisionShape) -> Self {
        Self {
            shape,
            component: None,
        }
    }

    pub fn cube() -> Self {
        Self::new(CollisionShape::CUBE())
    }

    pub fn sphere() -> Self {
        Self::new(CollisionShape::SPHERE())
    }
}

impl Component for CollisionShapeComponent {
    fn name(&self) -> &'static str {
        "collision_shape"
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

    fn to_mms_ast(&self) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        match self.shape {
            CollisionShape::Cube { half_extents } => ce_call(
                "CollisionShape",
                "cube",
                vec![array(nums(half_extents.iter().map(|&v| v as f64)))],
            ),
            CollisionShape::Sphere { radius } => {
                ce_call("CollisionShape", "sphere", vec![num(radius as f64)])
            }
        }
    }
}
