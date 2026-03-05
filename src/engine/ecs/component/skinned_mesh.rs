use crate::engine::ecs::component::Component;
use crate::engine::graphics::SkinId;

/// Runtime skinning metadata for a renderable.
///
/// Intended to be attached as a descendant of a `RenderableComponent`.
///
/// This is per-renderable because the computed skinning palette depends on the mesh's world
/// transform (we compute mesh-local skinning matrices).
#[derive(Debug, Clone)]
pub struct SkinnedMeshComponent {
    /// glTF skin index within the source asset.
    pub skin_index: usize,

    /// Runtime-only: reference to a World-owned skin instance.
    ///
    /// This avoids duplicating joint/IBM arrays for every primitive/renderable.
    pub skin_id: Option<SkinId>,
}

impl SkinnedMeshComponent {
    pub fn new(skin_index: usize) -> Self {
        Self {
            skin_index,
            skin_id: None,
        }
    }
}

impl Component for SkinnedMeshComponent {
    fn name(&self) -> &'static str {
        "skinned_mesh"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(
        &mut self,
        _emit: &mut dyn crate::engine::ecs::SignalEmitter,
        _component: crate::engine::ecs::ComponentId,
    ) {
        // No-op: SkinnedMeshSystem discovers these each frame.
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        // Runtime-only; avoid serializing ComponentId references.
        let mut map = std::collections::HashMap::new();
        map.insert(
            "skin_index".to_string(),
            serde_json::json!(self.skin_index as u32),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("skin_index") {
            let idx: u32 = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode skin_index: {}", e))?;
            self.skin_index = idx as usize;
        }

        // Runtime-only reference.
        self.skin_id = None;
        Ok(())
    }
}
