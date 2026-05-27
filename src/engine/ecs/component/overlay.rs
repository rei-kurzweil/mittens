use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Marks a subtree as belonging to the overlay render phase.
///
/// Any renderables under an `OverlayComponent` ancestor are routed into the overlay pass,
/// which is drawn after all other passes.
#[derive(Debug, Default, Clone, Copy)]
pub struct OverlayComponent {
    component: Option<ComponentId>,
}

impl OverlayComponent {
    pub fn new() -> Self {
        Self { component: None }
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }
}

impl Component for OverlayComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "overlay"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
