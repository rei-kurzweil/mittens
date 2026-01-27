use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Per-instance color for a renderable.
///
/// Intended to be attached as a descendant of a `RenderableComponent`.
#[derive(Debug, Clone, Copy)]
pub struct ColorComponent {
    pub rgba: [f32; 4],
}

impl ColorComponent {
    pub fn new() -> Self {
        Self {
            rgba: [1.0, 1.0, 1.0, 1.0],
        }
    }

    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { rgba: [r, g, b, a] }
    }

    pub fn with_rgba(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.rgba = [r, g, b, a];
        self
    }
}

impl Default for ColorComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ColorComponent {
    fn name(&self) -> &'static str {
        "color"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, queue: &mut crate::engine::ecs::CommandQueue, component: ComponentId) {
        queue.queue_register_color(component);
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("rgba".to_string(), serde_json::json!(self.rgba));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(rgba) = data.get("rgba") {
            self.rgba = serde_json::from_value(rgba.clone())
                .map_err(|e| format!("Failed to decode rgba: {}", e))?;
        }
        Ok(())
    }
}
