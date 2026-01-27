use super::Component;
use crate::engine::ecs::ComponentId;

/// Point light (local omnidirectional light).
///
/// This is purely an ECS representation for now; renderer integration will come later.
#[derive(Debug, Clone, Copy)]
pub struct PointLightComponent {
    pub intensity: f32,
    pub distance: f32,
    /// Linear RGB color in 0..1.
    pub color: [f32; 3],

    component: Option<ComponentId>,
}

impl PointLightComponent {
    pub fn new() -> Self {
        Self {
            intensity: 1.0,
            distance: 10.0,
            color: [1.0, 1.0, 1.0],
            component: None,
        }
    }

    pub fn with_intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity;
        self
    }

    pub fn with_distance(mut self, distance: f32) -> Self {
        self.distance = distance;
        self
    }

    pub fn with_color(mut self, r: f32, g: f32, b: f32) -> Self {
        self.color = [r, g, b];
        self
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }
}

impl Default for PointLightComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for PointLightComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "point_light"
    }

    fn init(
        &mut self,
        queue: &mut crate::engine::ecs::CommandQueue,
        component: crate::engine::ecs::ComponentId,
    ) {
        queue.queue_register_light(component);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("intensity".to_string(), serde_json::json!(self.intensity));
        map.insert("distance".to_string(), serde_json::json!(self.distance));
        map.insert("color".to_string(), serde_json::json!(self.color));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(intensity) = data.get("intensity") {
            self.intensity = serde_json::from_value(intensity.clone())
                .map_err(|e| format!("Failed to decode intensity: {}", e))?;
        }
        if let Some(distance) = data.get("distance") {
            self.distance = serde_json::from_value(distance.clone())
                .map_err(|e| format!("Failed to decode distance: {}", e))?;
        }
        if let Some(color) = data.get("color") {
            self.color = serde_json::from_value(color.clone())
                .map_err(|e| format!("Failed to decode color: {}", e))?;
        }
        Ok(())
    }
}
