use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Marks and owns the state for a world/component-tree panel.
///
/// Topology (spawned by `InspectorSystem::setup_panels_for_editor`):
///
/// ```text
/// SelectableComponent::off()     ← panel excluded from scene picking
///   OverlayComponent             ← always-on-top rendering
///     WorldPanelComponent        ← this component (stores rows_track + row_roots)
///       TransformComponent       ← rows_track: moved by ScrollingSystem
///         [row TransformComponents added dynamically]
/// ```
#[derive(Debug, Default, Clone)]
pub struct WorldPanelComponent {
    /// The editor root this panel belongs to.
    pub editor_root: Option<ComponentId>,

    /// Runtime: TransformComponent that row content is attached to.
    pub(crate) rows_track: Option<ComponentId>,

    /// Runtime: LayoutComponent (child of rows_track) that LayoutSystem uses to
    /// measure and position row TCs.
    pub(crate) rows_layout: Option<ComponentId>,

    /// Runtime: current row root TransformComponents (for cleanup on rebuild).
    pub(crate) row_roots: Vec<ComponentId>,

    /// Runtime: parallel to `row_roots` — the scene node each row represents.
    pub(crate) row_to_node: Vec<ComponentId>,

    component: Option<ComponentId>,
}

impl WorldPanelComponent {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Component for WorldPanelComponent {
    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
    }

    fn name(&self) -> &'static str {
        "world_panel"
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
