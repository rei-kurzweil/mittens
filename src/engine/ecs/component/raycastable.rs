use crate::engine::ecs::component::Component;

/// Controls whether renderables should be eligible for ray casting (BVH insertion).
///
/// This is intentionally separate from `RenderableComponent` so raycasting policy can be
/// expressed via topology/components rather than renderable data.
#[derive(Debug, Default, Clone, Copy)]
pub struct RaycastableComponent {
    /// If true, ray casting is enabled.
    pub enable: bool,
}

impl RaycastableComponent {
    pub fn new(enable: bool) -> Self {
        Self { enable }
    }

    pub fn enabled() -> Self {
        Self::new(true)
    }

    pub fn disabled() -> Self {
        Self::new(false)
    }
}

impl Component for RaycastableComponent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "raycastable"
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("enable".to_string(), serde_json::json!(self.enable));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("enable") {
            if let Some(b) = v.as_bool() {
                self.enable = b;
            }
        }
        // Backward-compatible: older saves may include "set_default"; ignore it.
        Ok(())
    }
}
