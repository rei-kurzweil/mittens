use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Load and spawn content from a glTF asset.
///
/// Attach this component somewhere under a `TransformComponent` to use that transform as an anchor.
#[derive(Debug, Clone)]
pub struct GLTFComponent {
    /// Path/URI to a `.gltf` or `.glb` asset (currently treated as local filesystem path).
    pub uri: String,

    /// Runtime-only: used by GLTFSystem to avoid re-spawning the same asset repeatedly.
    pub spawned: bool,

    component: Option<ComponentId>,
}

impl GLTFComponent {
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            spawned: false,
            component: None,
        }
    }
}

impl Component for GLTFComponent {
    fn name(&self) -> &'static str {
        "gltf"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
        let _ = self.component;
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, _queue: &mut crate::engine::ecs::CommandQueue, _component: ComponentId) {
        // No-op: GLTFSystem discovers these during tick().
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("uri".to_string(), serde_json::json!(self.uri));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(uri) = data.get("uri") {
            self.uri = serde_json::from_value(uri.clone())
                .map_err(|e| format!("Failed to decode uri: {}", e))?;
        }
        self.spawned = false;
        Ok(())
    }
}
