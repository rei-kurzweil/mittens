use crate::engine::ecs::component::Component;

#[derive(Debug, Clone)]
pub struct BlurPassComponent {
    pub enabled: bool,
    pub radius_ndc: f32,
    pub half_res: bool,
}

impl Default for BlurPassComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl BlurPassComponent {
    pub fn new() -> Self {
        let cfg = crate::engine::graphics::BlurPassConfig::default();
        Self {
            enabled: true,
            radius_ndc: cfg.radius_ndc,
            half_res: cfg.half_res,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_radius_ndc(mut self, radius_ndc: f32) -> Self {
        if radius_ndc.is_finite() {
            self.radius_ndc = radius_ndc.max(0.0);
        }
        self
    }

    pub fn with_half_res(mut self, half_res: bool) -> Self {
        self.half_res = half_res;
        self
    }
}

impl Component for BlurPassComponent {
    fn name(&self) -> &'static str {
        "blur_pass"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(&self) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce("BlurPass")
            .with_call("enabled", vec![b(self.enabled)])
            .with_call("radius_ndc", vec![num(self.radius_ndc as f64)])
            .with_call("half_res", vec![b(self.half_res)])
    }
}
