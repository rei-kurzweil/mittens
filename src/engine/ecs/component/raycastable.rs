use crate::engine::ecs::component::Component;

/// Controls whether renderables should be eligible for ray casting (BVH insertion).
///
/// This is intentionally separate from `RenderableComponent` so raycasting policy can be
/// expressed via topology/components rather than renderable data.
#[derive(Debug, Default, Clone, Copy)]
pub struct RaycastableComponent {
    /// If true, ray casting is enabled.
    pub enable: bool,

    /// If true, this component sets the *default* raycasting policy for renderables that do not
    /// have a RaycastableComponent.
    pub set_default: bool,
}

impl RaycastableComponent {
    pub fn new(enable: bool) -> Self {
        Self {
            enable,
            set_default: false,
        }
    }

    pub fn enabled() -> Self {
        Self::new(true)
    }

    pub fn disabled() -> Self {
        Self::new(false)
    }

    pub fn with_set_default(mut self, set_default: bool) -> Self {
        self.set_default = set_default;
        self
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
        map.insert(
            "set_default".to_string(),
            serde_json::json!(self.set_default),
        );
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
        if let Some(v) = data.get("set_default") {
            if let Some(b) = v.as_bool() {
                self.set_default = b;
            }
        }
        Ok(())
    }
}
