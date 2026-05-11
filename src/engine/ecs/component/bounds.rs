use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use crate::engine::graphics::bounds::Aabb;

/// Cached local-space AABB for a sibling `RenderableComponent`.
///
/// Attached automatically as a child of a renderable during
/// `SystemWorld::register_renderable` whenever the renderable's mesh has a
/// known local AABB (see `graphics::bounds::mesh_local_aabb`). Layout reads
/// this to size containers around renderable children without having to mutate
/// the renderable's own transform.
pub struct BoundsComponent {
    pub local: Aabb,
}

impl BoundsComponent {
    pub fn new(local: Aabb) -> Self {
        Self { local }
    }
}

impl Component for BoundsComponent {
    fn name(&self) -> &'static str {
        "bounds"
    }

    fn set_id(&mut self, _component: ComponentId) {}

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("min".to_string(), serde_json::json!(self.local.min));
        map.insert("max".to_string(), serde_json::json!(self.local.max));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("min") {
            self.local.min = serde_json::from_value(v.clone()).map_err(|e| format!("min: {e}"))?;
        }
        if let Some(v) = data.get("max") {
            self.local.max = serde_json::from_value(v.clone()).map_err(|e| format!("max: {e}"))?;
        }
        Ok(())
    }
}
