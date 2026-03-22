use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Selection opt-out marker.
///
/// Everything in an editor subtree is selectable by default. Wrapping a subtree in
/// `SelectableComponent::off()` excludes it from editor selection — clicking a descendant
/// will not move the gizmo, update the inspector context, or trigger `SelectionChanged`.
///
/// Used by `WorldPanelComponent` and `InspectorPanelComponent` to self-exclude panel UI from
/// scene picking.
#[derive(Debug, Clone, Copy)]
pub struct SelectableComponent {
    pub enabled: bool,
    component: Option<ComponentId>,
}

impl SelectableComponent {
    pub fn on() -> Self {
        Self { enabled: true, component: None }
    }

    pub fn off() -> Self {
        Self { enabled: false, component: None }
    }
}

impl Default for SelectableComponent {
    fn default() -> Self {
        Self::on()
    }
}

impl Component for SelectableComponent {
    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
    }

    fn name(&self) -> &'static str {
        "selectable"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut m = std::collections::HashMap::new();
        m.insert("enabled".to_string(), serde_json::json!(self.enabled));
        m
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("enabled") {
            self.enabled =
                serde_json::from_value(v.clone()).map_err(|e| format!("selectable.enabled: {e}"))?;
        }
        Ok(())
    }
}
