use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use crate::engine::graphics::CameraTarget;

#[derive(Debug, Clone)]
pub struct RendererStatsComponent {
    pub enabled: bool,

    /// Which renderer timing source this stats widget displays.
    ///
    /// This is explicit (no scene traversal): set it to `CameraTarget::Window` or
    /// `CameraTarget::Xr`.
    pub target: CameraTarget,

    /// Minimum time between text updates.
    ///
    /// Updating text is relatively expensive because it rebuilds glyph subtrees.
    pub update_interval_sec: f32,

    /// Exponential moving-average smoothing factor in $[0, 1]$.
    ///
    /// - 0.0: no smoothing
    /// - 0.9: heavy smoothing (default)
    pub smoothing: f32,

    /// Text color (RGBA).
    pub color: [f32; 4],

    /// Whether spawned glyphs should be emissive (unlit).
    pub emissive: bool,

    // --- runtime-only state (not serialized) ---
    component_id: Option<ComponentId>,
    time_since_update_sec: f32,
    smoothed_fps: Option<f32>,

    // Auto-managed subtree ids.
    text: Option<ComponentId>,
    text_color: Option<ComponentId>,
    text_emissive: Option<ComponentId>,
}

impl RendererStatsComponent {
    pub fn new() -> Self {
        Self {
            enabled: true,
            target: CameraTarget::Window,
            update_interval_sec: 0.25,
            smoothing: 0.9,
            color: [1.0, 1.0, 1.0, 1.0],
            emissive: true,

            component_id: None,
            time_since_update_sec: 0.0,
            smoothed_fps: None,

            text: None,
            text_color: None,
            text_emissive: None,
        }
    }

    pub fn with_camera_target(mut self, target: CameraTarget) -> Self {
        self.target = target;
        self
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component_id
    }

    pub fn accumulate_time(&mut self, dt_sec: f32) {
        if !self.enabled {
            return;
        }
        self.time_since_update_sec = (self.time_since_update_sec + dt_sec).max(0.0);
    }

    pub fn should_update(&self) -> bool {
        if !self.enabled {
            return false;
        }
        self.time_since_update_sec >= self.update_interval_sec.max(0.0)
    }

    pub fn reset_update_timer(&mut self) {
        self.time_since_update_sec = 0.0;
    }

    pub fn smooth_fps(&mut self, fps: f32) -> f32 {
        let fps = if fps.is_finite() { fps.max(0.0) } else { 0.0 };
        let s = self.smoothing.clamp(0.0, 1.0);
        let out = match self.smoothed_fps {
            None => fps,
            Some(prev) => prev * s + fps * (1.0 - s),
        };
        self.smoothed_fps = Some(out);
        out
    }

    pub(crate) fn runtime_subtree_ids_mut(
        &mut self,
    ) -> (
        &mut Option<ComponentId>,
        &mut Option<ComponentId>,
        &mut Option<ComponentId>,
    ) {
        (
            &mut self.text,
            &mut self.text_color,
            &mut self.text_emissive,
        )
    }
}

impl Default for RendererStatsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for RendererStatsComponent {
    fn name(&self) -> &'static str {
        "renderer_stats"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component_id = Some(component);
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
        let target_str = match self.target {
            CameraTarget::Window => "Window",
            CameraTarget::Xr => "Xr",
        };
        ce("RendererStats")
            .with_call("enabled", vec![b(self.enabled)])
            .with_call("camera_target", vec![s(target_str)])
            .with_call(
                "update_interval_sec",
                vec![num(self.update_interval_sec as f64)],
            )
            .with_call("smoothing", vec![num(self.smoothing as f64)])
            .with_call(
                "color",
                vec![array(nums(self.color.iter().map(|&v| v as f64)))],
            )
            .with_call("emissive", vec![b(self.emissive)])
    }
}
