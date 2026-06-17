use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Mirror component.
///
/// Attached to a `RenderableComponent` (or its ancestor) to enable planar reflections.
/// The mirror plane is defined by the nearest ancestor `TransformComponent`'s XY plane
/// (+Z is the reflection normal).
#[derive(Debug, Clone)]
pub struct MirrorComponent {
    /// Resolution of the mirror texture (e.g. 512, 1024).
    pub quality: i32,
    component: Option<ComponentId>,
}

impl MirrorComponent {
    pub fn new(quality: i32) -> Self {
        Self {
            quality: quality.clamp(64, 2048),
            component: None,
        }
    }
}

impl Default for MirrorComponent {
    fn default() -> Self {
        Self::new(512)
    }
}

impl Component for MirrorComponent {
    fn name(&self) -> &'static str {
        "mirror"
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

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call("Mirror", "quality", vec![num(self.quality as f64)])
    }
}
