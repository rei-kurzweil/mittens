use super::Component;
use crate::engine::ecs::ComponentId;

/// Marker/debug component for glTF joint transforms.
///
/// GLTFSystem attaches this as a direct child of a `TransformComponent` that is referenced
/// as a joint by at least one glTF skin.
#[derive(Debug, Clone)]
pub struct JointComponent {
    /// glTF node index for this joint.
    pub node_index: usize,

    /// glTF skin indices that reference this joint.
    pub skin_indices: Vec<usize>,
}

impl JointComponent {
    pub fn new(node_index: usize, skin_indices: Vec<usize>) -> Self {
        Self {
            node_index,
            skin_indices,
        }
    }
}

impl Component for JointComponent {
    fn name(&self) -> &'static str {
        "joint"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, _queue: &mut crate::engine::ecs::CommandQueue, _component: ComponentId) {
        // No-op.
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert(
            "node_index".to_string(),
            serde_json::json!(self.node_index as u32),
        );
        map.insert(
            "skin_indices".to_string(),
            serde_json::json!(
                self.skin_indices
                    .iter()
                    .copied()
                    .map(|i| i as u32)
                    .collect::<Vec<u32>>()
            ),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("node_index") {
            let idx: u32 = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode node_index: {}", e))?;
            self.node_index = idx as usize;
        }
        if let Some(v) = data.get("skin_indices") {
            let list: Vec<u32> = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode skin_indices: {}", e))?;
            self.skin_indices = list.into_iter().map(|i| i as usize).collect();
        }
        Ok(())
    }
}
