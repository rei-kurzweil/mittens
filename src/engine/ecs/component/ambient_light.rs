use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Global ambient light.
///
/// This is intended to be a singleton-like component (the last registered wins).
/// The value is linear RGB in 0..1.
#[derive(Debug, Clone, Copy)]
pub struct AmbientLightComponent {
    pub rgb: [f32; 3],
}

impl AmbientLightComponent {
    pub fn new() -> Self {
        Self {
            rgb: [0.0, 0.0, 0.0],
        }
    }

    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { rgb: [r, g, b] }
    }

    pub fn with_rgb(mut self, r: f32, g: f32, b: f32) -> Self {
        self.rgb = [r, g, b];
        self
    }
}

impl Default for AmbientLightComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for AmbientLightComponent {
    fn name(&self) -> &'static str {
        "ambient_light"
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
            crate::engine::ecs::IntentValue::RegisterAmbientLight {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(&self) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call("AmbientLight", "rgb", nums(self.rgb.iter().map(|&v| v as f64)))
    }
}
