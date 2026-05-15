use super::Component;

/// Strips pitch and roll from the rotation channel, keeping only the Y-axis component.
/// Equivalent to projecting the input quaternion onto the Y-rotation subspace:
/// `normalize([0, q.y, 0, q.w])`.
///
/// Convention-independent: the output is always a pure-Y quaternion whose angle
/// equals the Y-rotation of the input quaternion.
#[derive(Debug, Clone, Copy, Default)]
pub struct QuatExtractYawComponent;

impl QuatExtractYawComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for QuatExtractYawComponent {
    fn name(&self) -> &'static str {
        "quat_extract_yaw"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Stateful body-yaw follow: extracts world-Y yaw from the input rotation, then
/// advances a running `body_yaw` toward the head yaw when the delta exceeds
/// `threshold`, at `rate` rad/s. Outputs a pure-Y quaternion for `body_yaw`.
///
/// State lives in `TransformPipelineSystem` alongside `QuatTemporalFilter` state,
/// keyed by stage path.
///
/// `forward_plus_z` controls the yaw-extraction convention:
/// - `false` (default, OpenXR/VR): -Z forward; at identity rotation yaw = π.
/// - `true` (desktop): +Z forward; at identity rotation yaw = 0.
///
/// `initial_yaw` seeds `body_yaw` on the first frame. Set to `π` for VR so the
/// body starts aligned with OpenXR's -Z-forward rest pose.
#[derive(Debug, Clone, Copy)]
pub struct QuatYawFollowComponent {
    pub threshold: f32,
    pub rate: f32,
    pub forward_plus_z: bool,
    pub initial_yaw: f32,
}

impl QuatYawFollowComponent {
    pub fn new(threshold: f32, rate: f32) -> Self {
        Self {
            threshold,
            rate,
            forward_plus_z: false,
            initial_yaw: 0.0,
        }
    }

    pub fn with_forward_plus_z(mut self) -> Self {
        self.forward_plus_z = true;
        self
    }

    pub fn with_forward_plus_z_if(mut self, value: bool) -> Self {
        self.forward_plus_z = value;
        self
    }

    pub fn with_initial_yaw(mut self, yaw: f32) -> Self {
        self.initial_yaw = yaw;
        self
    }
}

impl Default for QuatYawFollowComponent {
    fn default() -> Self {
        Self::new(std::f32::consts::FRAC_PI_4, 3.0)
    }
}

impl Component for QuatYawFollowComponent {
    fn name(&self) -> &'static str {
        "quat_yaw_follow"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(&self) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut c = ce_call(
            "QuatYawFollow",
            "new",
            vec![num(self.threshold as f64), num(self.rate as f64)],
        );
        if self.forward_plus_z {
            c = c.with_call("forward_plus_z", vec![]);
        }
        if self.initial_yaw != 0.0 {
            c = c.with_call("initial_yaw", vec![num(self.initial_yaw as f64)]);
        }
        c
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Vector3TemporalFilterComponent {
    pub smoothing_factor: f32,
}

impl Vector3TemporalFilterComponent {
    pub fn new() -> Self {
        Self {
            smoothing_factor: 1.0,
        }
    }

    pub fn with_smoothing_factor(mut self, smoothing_factor: f32) -> Self {
        self.smoothing_factor = smoothing_factor;
        self
    }
}

impl Default for Vector3TemporalFilterComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Vector3TemporalFilterComponent {
    fn name(&self) -> &'static str {
        "vector3_temporal_filter"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(&self) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call(
            "Vector3TemporalFilter",
            "smoothing_factor",
            vec![num(self.smoothing_factor as f64)],
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct QuatTemporalFilterComponent {
    pub smoothing_factor: f32,
}

impl QuatTemporalFilterComponent {
    pub fn new() -> Self {
        Self {
            smoothing_factor: 1.0,
        }
    }

    pub fn with_smoothing_factor(mut self, smoothing_factor: f32) -> Self {
        self.smoothing_factor = smoothing_factor;
        self
    }
}

impl Default for QuatTemporalFilterComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for QuatTemporalFilterComponent {
    fn name(&self) -> &'static str {
        "quat_temporal_filter"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(&self) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call(
            "QuatTemporalFilter",
            "smoothing_factor",
            vec![num(self.smoothing_factor as f64)],
        )
    }
}
