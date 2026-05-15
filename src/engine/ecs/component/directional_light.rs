use super::Component;
use crate::engine::ecs::ComponentId;

/// Directional light (infinite distance light).
///
/// The renderer interprets this light's *world position* as a direction vector.
/// In other words: set the node's translation to the direction you want (it will
/// be normalized on the GPU).
#[derive(Debug, Clone, Copy)]
pub struct DirectionalLightComponent {
    pub intensity: f32,
    /// Linear RGB color in 0..1.
    pub color: [f32; 3],

    component: Option<ComponentId>,
}

impl DirectionalLightComponent {
    pub fn new() -> Self {
        Self {
            intensity: 1.0,
            color: [1.0, 1.0, 1.0],
            component: None,
        }
    }

    pub fn with_intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity;
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

impl Default for DirectionalLightComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for DirectionalLightComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "directional_light"
    }

    fn init(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: crate::engine::ecs::ComponentId,
    ) {
        // Uses the same light registration path as point lights.
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterLight {
                component_ids: vec![component],
            },
        );
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(&self) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce("DirectionalLight")
            .with_call("intensity", vec![num(self.intensity as f64)])
            .with_call(
                "color",
                nums(self.color.iter().map(|&v| v as f64)),
            )
    }
}
