use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Marks and owns the state for a component inspector panel.
///
/// Topology (spawned by `InspectorSystem::setup_panels_for_editor`):
///
/// ```text
/// SelectableComponent::off()
///   OverlayComponent
///     InspectorPanelComponent    ← this component
///       TransformComponent       ← rows_track: moved by ScrollingSystem
///         [row TransformComponents added dynamically]
/// ```
#[derive(Debug, Default, Clone)]
pub struct InspectorPanelComponent {
    /// The editor root this panel belongs to.
    pub editor_root: Option<ComponentId>,

    /// Currently inspected component (drives panel content).
    pub inspected: Option<ComponentId>,

    /// Runtime: TransformComponent that row content is attached to.
    pub(crate) rows_track: Option<ComponentId>,

    /// Runtime: LayoutComponent (child of rows_track) that LayoutSystem uses to
    /// measure and position row TCs.
    pub(crate) rows_layout: Option<ComponentId>,

    /// Runtime: current row root TransformComponents (for cleanup on rebuild).
    pub(crate) row_roots: Vec<ComponentId>,

    component: Option<ComponentId>,
}

impl InspectorPanelComponent {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Component for InspectorPanelComponent {
    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
    }

    fn name(&self) -> &'static str {
        "inspector_panel"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(&self, _world: &crate::engine::ecs::World) -> crate::meow_meow::ast::ComponentExpression {
        crate::engine::ecs::component::ce_helpers::ce("InspectorPanel")
    }
}
