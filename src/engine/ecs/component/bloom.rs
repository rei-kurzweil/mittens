use crate::engine::ecs::component::Component;

#[derive(Debug, Clone)]
pub struct BloomComponent {
    pub enabled: bool,
    pub intensity: f32,
    pub radius_ndc: f32,
    pub emissive_scale: f32,
    pub half_res: bool,
    pub output_texture: Option<String>,
}

impl Default for BloomComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl BloomComponent {
    pub fn new() -> Self {
        let cfg = crate::engine::graphics::BloomConfig::default();
        Self {
            enabled: true,
            intensity: cfg.intensity,
            radius_ndc: cfg.radius_ndc,
            emissive_scale: cfg.emissive_scale,
            half_res: cfg.half_res,
            output_texture: None,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_intensity(mut self, intensity: f32) -> Self {
        if intensity.is_finite() {
            self.intensity = intensity.max(0.0);
        }
        self
    }

    pub fn with_radius_ndc(mut self, radius_ndc: f32) -> Self {
        if radius_ndc.is_finite() {
            self.radius_ndc = radius_ndc.max(0.0);
        }
        self
    }

    pub fn with_emissive_scale(mut self, emissive_scale: f32) -> Self {
        if emissive_scale.is_finite() {
            self.emissive_scale = emissive_scale.max(0.0);
        }
        self
    }

    pub fn with_half_res(mut self, half_res: bool) -> Self {
        self.half_res = half_res;
        self
    }

    pub fn with_output_texture(mut self, output_texture: impl Into<String>) -> Self {
        self.output_texture = Some(output_texture.into());
        self
    }
}

impl Component for BloomComponent {
    fn name(&self) -> &'static str {
        "bloom"
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
        let mut ce = ce("Bloom")
            .with_call("enabled", vec![b(self.enabled)])
            .with_call("intensity", vec![num(self.intensity as f64)])
            .with_call("radius_ndc", vec![num(self.radius_ndc as f64)])
            .with_call("emissive_scale", vec![num(self.emissive_scale as f64)])
            .with_call("half_res", vec![b(self.half_res)]);
        if let Some(tex) = &self.output_texture {
            ce = ce.with_call("output_texture", vec![s(tex)]);
        }
        ce
    }
}
