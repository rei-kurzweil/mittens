use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Select a mesh for a Renderable by string key.
///
/// This is intended to be attached as a descendant of a `RenderableComponent`.
/// The key can refer to imported meshes (e.g. "{gltf}:{mesh}:{prim}") registered in `RenderAssets`.
#[derive(Debug, Clone)]
pub struct MeshComponent {
    pub key: String,
}

impl MeshComponent {
    pub fn new(key: impl Into<String>) -> Self {
        Self { key: key.into() }
    }
}

impl Component for MeshComponent {
    fn name(&self) -> &'static str {
        "mesh"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, _emit: &mut dyn crate::engine::ecs::SignalEmitter, _component: ComponentId) {
        // No-op: RenderableSystem resolves this opportunistically during flush.
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("key".to_string(), serde_json::json!(self.key));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(key) = data.get("key") {
            self.key = serde_json::from_value(key.clone())
                .map_err(|e| format!("Failed to decode key: {}", e))?;
        }
        Ok(())
    }
}
