use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Marker component: route this renderable into the "transparent cutout" pass.
///
/// Intended to be attached as a descendant of a `RenderableComponent`.
///
/// Semantics:
/// - Uses alpha-to-coverage (MSAA) instead of blending.
/// - Depth test/write stays enabled, so it behaves like opaque geometry for ordering.
#[derive(Debug, Clone, Copy)]
pub struct TransparentCutoutComponent {
    pub enabled: bool,
}

impl TransparentCutoutComponent {
    pub fn new() -> Self {
        Self { enabled: true }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Default for TransparentCutoutComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for TransparentCutoutComponent {
    fn name(&self) -> &'static str {
        "transparent_cutout"
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
            crate::engine::ecs::IntentValue::RegisterTransparentCutout { component },
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
        if let Some(v) = data.get("enabled") {
            if let Some(b) = v.as_bool() {
                self.enabled = b;
            }
        }
        Ok(())
    }
}
