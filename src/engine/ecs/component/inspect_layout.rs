use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Marks a [`LayoutComponent`] subtree as wanting box-model visualisation.
///
/// When attached as a child of a TC that also carries a `LayoutComponent`, the
/// layout pass over that subtree spawns box-model viz quads (`__box_padding_*`,
/// `__box_content`, `__box_margin_*`) per styled item. Without this component,
/// layout skips viz and tears down any pre-existing viz subtrees so the panel
/// renders clean.
///
/// Per-LayoutRoot, not global: each layout tree opts in independently.
///
/// [`LayoutComponent`]: crate::engine::ecs::component::LayoutComponent
#[derive(Debug, Default, Clone, Copy)]
pub struct InspectLayoutComponent {
    component: Option<ComponentId>,
}

impl InspectLayoutComponent {
    pub fn new() -> Self {
        Self { component: None }
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }
}

impl Component for InspectLayoutComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "inspect_layout"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
