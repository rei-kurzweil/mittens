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
}

impl RendererSettingsComponent {
    pub fn new() -> Self {
        Self {
            msaa4x: true,
            window_size: None,
        }
    }

    pub fn msaa_off() -> Self {
        Self {
            msaa4x: false,
            window_size: None,
        }
    }

    pub fn with_window_size(mut self, width: u32, height: u32) -> Self {
        if width > 0 && height > 0 {
            self.window_size = Some([width, height]);
        }
        self
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
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(&self) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut ce = if self.msaa4x {
            ce("RendererSettings")
        } else {
            ce_call("RendererSettings", "msaa_off", vec![])
        };
        if let Some([w, h]) = self.window_size {
            ce = ce.with_call("window_size", vec![num(w as f64), num(h as f64)]);
        }
        ce
    }
}
