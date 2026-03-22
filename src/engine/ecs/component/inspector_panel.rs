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
///       TransformComponent       ← rows_anchor: world-space position
///         [row TransformComponents added dynamically]
/// ```
#[derive(Debug, Default, Clone)]
pub struct InspectorPanelComponent {
    /// The editor root this panel belongs to.
    pub editor_root: Option<ComponentId>,

    /// Currently inspected component (drives panel content).
    pub inspected: Option<ComponentId>,

    /// First visible row index (for scrolling).
    pub scroll_offset_rows: i32,

    /// Runtime: TransformComponent that row rows are attached to.
    pub(crate) rows_anchor: Option<ComponentId>,

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

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        std::collections::HashMap::new()
    }

    fn decode(
        &mut self,
        _data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        Ok(())
    }
}
