use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Per-instance opacity multiplier for a renderable.
///
/// Intended to be attached as a descendant of a `RenderableComponent`.
///
/// Note: opacity is a *multiplier* (0..1). It combines with the instance color alpha and
/// any sampled texture alpha in the shader.
#[derive(Debug, Clone, Copy)]
pub struct OpacityComponent {
    pub opacity: f32,
    /// If true, this renderable should be treated as requiring correct multi-layer blending.
    ///
    /// This routes the instance into the sorted (slow) transparent pass.
    /// When false, the instance can use the instanced (fast) transparent pass.
    pub multiple_layers: bool,
}

impl OpacityComponent {
    pub fn new() -> Self {
        Self {
            opacity: 1.0,
            multiple_layers: false,
        }
    }

    /// Convenience: set opacity from an 8-bit value (0..255).
    pub fn with_value(mut self, value: u8) -> Self {
        self.opacity = (value as f32) / 255.0;
        self
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self
    }

    /// Mark this opacity as requiring correct multi-layer blending.
    ///
    /// This opts the renderable into the sorted transparent pass (no instancing).
    pub fn with_multiple_layers(mut self) -> Self {
        self.multiple_layers = true;
        self
    }
}

impl Default for OpacityComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for OpacityComponent {
    fn name(&self) -> &'static str {
        "opacity"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, queue: &mut crate::engine::ecs::CommandQueue, component: ComponentId) {
        queue.queue_register_opacity(component);
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("opacity".to_string(), serde_json::json!(self.opacity));
        map.insert(
            "multiple_layers".to_string(),
            serde_json::json!(self.multiple_layers),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("opacity") {
            // Accept either a float (0..1) or an integer (0..255) for convenience.
            if let Some(f) = v.as_f64() {
                self.opacity = (f as f32).clamp(0.0, 1.0);
            } else if let Some(i) = v.as_i64() {
                let i = i.clamp(0, 255) as u8;
                self.opacity = (i as f32) / 255.0;
            }
        }

        if let Some(v) = data.get("multiple_layers") {
            if let Some(b) = v.as_bool() {
                self.multiple_layers = b;
            }
        }
        Ok(())
    }
}
