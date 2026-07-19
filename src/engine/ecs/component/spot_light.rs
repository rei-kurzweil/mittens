use super::Component;
use crate::engine::ecs::ComponentId;

/// Cone-shaped local light aimed along its transform's local +Z axis.
#[derive(Debug, Clone, Copy)]
pub struct SpotLightComponent {
    pub intensity: f32,
    pub distance: f32,
    /// Outer half-angle of the cone, in radians.
    pub angle: f32,
    /// Fraction of the cone used to soften its edge (`0` = hard, `1` = fully soft).
    pub penumbra: f32,
    /// Linear RGB color in 0..1.
    pub color: [f32; 3],

    component: Option<ComponentId>,
}

impl SpotLightComponent {
    pub fn new() -> Self {
        Self {
            intensity: 1.0,
            distance: 10.0,
            angle: std::f32::consts::FRAC_PI_4,
            penumbra: 0.1,
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

    pub fn with_angle(mut self, angle: f32) -> Self {
        self.angle = angle;
        self
    }

    pub fn with_penumbra(mut self, penumbra: f32) -> Self {
        self.penumbra = penumbra;
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

impl Default for SpotLightComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for SpotLightComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "spot_light"
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
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

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce("SpotLight")
            .with_call("intensity", vec![num(self.intensity as f64)])
            .with_call("distance", vec![num(self.distance as f64)])
            .with_call("angle", vec![num(self.angle as f64)])
            .with_call("penumbra", vec![num(self.penumbra as f64)])
            .with_call("color", nums(self.color.iter().map(|&v| v as f64)))
    }
}
