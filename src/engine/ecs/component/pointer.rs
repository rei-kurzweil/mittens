use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Opt-in marker/config that says a raycaster participates in pointer-driven interaction.
///
/// Intended topology:
/// - attach `PointerComponent` as a child of a `RayCastComponent` (or to the same parent),
///   so systems can treat that raycaster as a user-facing pointer.
///
/// This is intentionally small for now; it gives us a stable "pointer_id" concept in the
/// component tree without forcing every raycaster to behave like a pointer.
#[derive(Debug, Clone, Copy)]
pub struct PointerComponent {
    pub enabled: bool,

    component: Option<ComponentId>,
}

impl PointerComponent {
    pub fn new() -> Self {
        Self {
            enabled: true,
            component: None,
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            component: None,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Default for PointerComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for PointerComponent {
    fn name(&self) -> &'static str {
        "pointer"
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

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("enabled".to_string(), serde_json::json!(self.enabled));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(enabled) = data.get("enabled") {
            self.enabled = serde_json::from_value(enabled.clone())
                .map_err(|e| format!("Failed to decode enabled: {}", e))?;
        }
        Ok(())
    }
}
