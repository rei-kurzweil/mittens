use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Load and spawn content from a glTF asset.
///
/// Attach this component somewhere under a `TransformComponent` to use that transform as an anchor.
#[derive(Debug, Clone)]
pub struct GLTFComponent {
    /// Path/URI to a `.gltf` or `.glb` asset (currently treated as local filesystem path).
    pub uri: String,

    /// If true, GLTFSystem will give transform-only nodes a small debug renderable.
    ///
    /// This is useful for editor-style workflows where you want to see and grab node transforms
    /// even when the node has no mesh.
    pub with_visualized_transforms: bool,

    /// Runtime-only: used by GLTFSystem to avoid re-spawning the same asset repeatedly.
    pub spawned: bool,

    component: Option<ComponentId>,
}

impl GLTFComponent {
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            with_visualized_transforms: false,
            spawned: false,
            component: None,
        }
    }

    pub fn with_visualized_transforms(mut self, with_visualized_transforms: bool) -> Self {
        self.with_visualized_transforms = with_visualized_transforms;
        self
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

    fn init(&mut self, _emit: &mut dyn crate::engine::ecs::SignalEmitter, _component: ComponentId) {
        // No-op: GLTFSystem discovers these during tick().
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("uri".to_string(), serde_json::json!(self.uri));
        map.insert(
            "with_visualized_transforms".to_string(),
            serde_json::json!(self.with_visualized_transforms),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let uri = data
            .get("uri")
            .ok_or_else(|| "Missing required field: uri".to_string())?;
        self.uri = serde_json::from_value(uri.clone())
            .map_err(|e| format!("Failed to decode uri: {e}"))?;

        let with_visualized_transforms = data
            .get("with_visualized_transforms")
            .ok_or_else(|| "Missing required field: with_visualized_transforms".to_string())?;
        self.with_visualized_transforms = serde_json::from_value(with_visualized_transforms.clone())
            .map_err(|e| format!("Failed to decode with_visualized_transforms: {e}"))?;

        self.spawned = false;
        Ok(())
    }
}
