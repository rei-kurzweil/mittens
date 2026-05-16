use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

use crate::engine::ecs::system::model::collision_types::CollisionMode;

/// Enables collision participation for an entity.
///
/// Shape can be provided via a child `CollisionShapeComponent`.
/// If absent, the CollisionSystem will try to infer a shape from a sibling `RenderableComponent`
/// using known built-in meshes (initially cube/sphere only).
#[derive(Debug, Clone)]
pub struct CollisionComponent {
    pub mode: CollisionMode,

    component: Option<ComponentId>,
}

impl CollisionComponent {
    pub fn new(mode: CollisionMode) -> Self {
        Self {
            mode,
            component: None,
        }
    }

    #[allow(non_snake_case)]
    pub fn STATIC() -> Self {
        Self::new(CollisionMode::Static)
    }

    #[allow(non_snake_case)]
    pub fn KINEMATIC() -> Self {
        Self::new(CollisionMode::Kinematic)
    }

    #[allow(non_snake_case)]
    pub fn RIGGED() -> Self {
        Self::new(CollisionMode::Rigged)
    }
}

impl Default for CollisionComponent {
    fn default() -> Self {
        Self::STATIC()
    }
}

impl Component for CollisionComponent {
    fn name(&self) -> &'static str {
        "collision"
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

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterCollision {
                component_ids: vec![component],
            },
        );
    }

    fn cleanup(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: ComponentId,
    ) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RemoveCollision {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(&self, _world: &crate::engine::ecs::World) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = match self.mode {
            CollisionMode::Static => "static",
            CollisionMode::Kinematic => "kinematic",
            CollisionMode::Rigged => "rigged",
        };
        ce_call("Collision", ctor, vec![])
    }
}
