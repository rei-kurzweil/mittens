use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use crate::engine::graphics::MsaaMode;

/// Global renderer settings.
///
/// This is intended to be a singleton-like component (the last registered wins).
#[derive(Debug, Clone, Copy)]
pub struct RendererSettingsComponent {
    pub msaa4x: bool,
    pub window_size: Option<[u32; 2]>,
    /// Disable the desktop host window/surface while simulation or XR continues.
    pub window_visible: bool,
    /// Desired render size in pixels for each XR eye, selected at session creation.
    pub desired_xr_resolution: Option<[u32; 2]>,
    /// Preferred display rate in Hz; unsupported rates leave the runtime default.
    pub preferred_xr_refresh_rate: Option<f32>,
    pub transmission_depth_compare: bool,
}

impl RendererSettingsComponent {
    pub fn new() -> Self {
        Self {
            msaa4x: true,
            window_size: None,
            window_visible: true,
            desired_xr_resolution: None,
            preferred_xr_refresh_rate: None,
            transmission_depth_compare: true,
        }
    }

    pub fn msaa_off() -> Self {
        Self {
            msaa4x: false,
            window_size: None,
            window_visible: true,
            desired_xr_resolution: None,
            preferred_xr_refresh_rate: None,
            transmission_depth_compare: true,
        }
    }

    pub fn with_window_size(mut self, width: u32, height: u32) -> Self {
        if width > 0 && height > 0 {
            self.window_size = Some([width, height]);
        }
        self
    }

    pub fn with_window_visible(mut self, visible: bool) -> Self {
        self.window_visible = visible;
        self
    }

    pub fn with_transmission_depth_compare(mut self, enabled: bool) -> Self {
        self.transmission_depth_compare = enabled;
        self
    }

    pub fn with_desired_xr_resolution(mut self, resolution: [u32; 2]) -> Self {
        if resolution[0] > 0 && resolution[1] > 0 {
            self.desired_xr_resolution = Some(resolution);
        }
        self
    }

    pub fn validate_xr_resolution(value: [f64; 2]) -> Result<[u32; 2], String> {
        if value
            .iter()
            .any(|v| !v.is_finite() || *v < 1.0 || *v > u32::MAX as f64 || v.fract() != 0.0)
        {
            return Err(
                "desired_xr_resolution requires two positive integer pixel dimensions".into(),
            );
        }
        Ok(value.map(|v| v as u32))
    }

    pub fn with_preferred_xr_refresh_rate(mut self, hz: f32) -> Result<Self, String> {
        if !hz.is_finite() || hz <= 0.0 {
            return Err("preferred_xr_refresh_rate requires a positive finite rate in Hz".into());
        }
        self.preferred_xr_refresh_rate = Some(hz);
        Ok(self)
    }

    pub fn msaa_mode(&self) -> MsaaMode {
        if self.msaa4x {
            MsaaMode::Msaa4x
        } else {
            MsaaMode::Off
        }
    }
}

impl Default for RendererSettingsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for RendererSettingsComponent {
    fn name(&self) -> &'static str {
        "renderer_settings"
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
            crate::engine::ecs::IntentValue::RegisterRendererSettings {
                component_id: component,
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut ce = if self.msaa4x {
            ce("RendererSettings")
        } else {
            ce_call("RendererSettings", "msaa_off", vec![])
        };
        if let Some([w, h]) = self.window_size {
            ce = ce.with_call("window_size", vec![num(w as f64), num(h as f64)]);
        }
        if !self.window_visible {
            ce = ce.with_call("window_visible", vec![b(false)]);
        }
        if let Some([w, h]) = self.desired_xr_resolution {
            ce = ce.with_call(
                "desired_xr_resolution",
                vec![array(vec![num(w as f64), num(h as f64)])],
            );
        }
        if !self.transmission_depth_compare {
            ce = ce.with_call("transmission_depth_compare", vec![b(false)]);
        }
        if let Some(hz) = self.preferred_xr_refresh_rate {
            ce = ce.with_call("preferred_xr_refresh_rate", vec![num(hz as f64)]);
        }
        ce
    }
}
