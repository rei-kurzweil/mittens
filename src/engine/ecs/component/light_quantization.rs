use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Per-renderable light quantization control for the toon shader.
///
/// This controls `MaterialUBO.quant_steps` (see `assets/shaders/toon-mesh.frag`).
/// Intended to be attached as a descendant of a `RenderableComponent`.
#[derive(Debug, Clone, Copy)]
pub struct LightQuantizationComponent {
    pub quant_steps: f32,
}

impl LightQuantizationComponent {
    /// Default toon quantization steps.
    pub const DEFAULT_STEPS: f32 = 3.0;

    pub fn new() -> Self {
        Self {
            quant_steps: Self::DEFAULT_STEPS,
        }
    }

    pub fn steps(steps: f32) -> Self {
        Self { quant_steps: steps }
    }

    pub fn with_steps(mut self, steps: f32) -> Self {
        self.quant_steps = steps;
        self
    }
}

impl Default for LightQuantizationComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for LightQuantizationComponent {
    fn name(&self) -> &'static str {
        "light_quantization"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterLightQuantization {
                component_ids: vec![component],
            },
        );
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert(
            "quant_steps".to_string(),
            serde_json::Value::Number(
                serde_json::Number::from_f64(self.quant_steps as f64).unwrap_or_else(|| {
                    // Fallback (NaN/inf): default.
                    serde_json::Number::from_f64(Self::DEFAULT_STEPS as f64).unwrap()
                }),
            ),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("quant_steps") {
            self.quant_steps = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode quant_steps: {}", e))?;
        }
        Ok(())
    }
}
