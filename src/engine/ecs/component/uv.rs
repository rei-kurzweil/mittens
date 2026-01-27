use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Per-vertex UVs for a renderable.
///
/// This is intended to be attached as a descendant of a `RenderableComponent`.
///
/// Lifecycle note:
/// - UV overrides are applied when the renderable is flushed into `VisualWorld` / uploaded.
/// - If fewer UVs are provided than the mesh's vertex count, the missing UVs are filled with 0.
#[derive(Debug, Clone)]
pub struct UVComponent {
    pub uvs: Vec<[f32; 2]>,
}

impl UVComponent {
    pub fn new() -> Self {
        Self { uvs: Vec::new() }
    }

    /// Construct from a nested vector, where each inner vec is `[u, v]`.
    ///
    /// - If an inner vec has <2 values, missing values are treated as 0.
    /// - If it has >2 values, extras are ignored.
    pub fn fromVec(uvs: Vec<Vec<f32>>) -> Self {
        let mut out: Vec<[f32; 2]> = Vec::with_capacity(uvs.len());
        for uv in uvs {
            let u = uv.get(0).copied().unwrap_or(0.0);
            let v = uv.get(1).copied().unwrap_or(0.0);
            out.push([u, v]);
        }
        Self { uvs: out }
    }

    pub fn with_uv(mut self, u: f32, v: f32) -> Self {
        self.uvs.push([u, v]);
        self
    }
}

impl Default for UVComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for UVComponent {
    fn name(&self) -> &'static str {
        "uv"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, queue: &mut crate::engine::ecs::CommandQueue, component: ComponentId) {
        queue.queue_register_uv(component);
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("uvs".to_string(), serde_json::json!(self.uvs));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(uvs) = data.get("uvs") {
            self.uvs = serde_json::from_value(uvs.clone())
                .map_err(|e| format!("Failed to decode uvs: {}", e))?;
        }
        Ok(())
    }
}
