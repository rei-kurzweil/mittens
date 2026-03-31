use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// User-facing pointer component.
///
/// Attach this under the pose-driving part of the topology (for example a desktop camera rig
/// transform, an XR camera, or a controller-driven transform). At init time the engine spawns
/// and owns a child `RayCastComponent`, so authoring only needs to describe the pointer itself.
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

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        self.component = Some(component);
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterPointer {
                component_ids: vec![component],
            },
        );
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
