use super::{AvatarControlComponent, Component, ce_helpers::*};

/// Optional direct child of AVC. Owns procedural walking tuning, not pose state.
/// Without this component, AVC's existing walking settings supply the base gait.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WalkingIkComponent {
    pub enabled: bool,
    /// Base maximum foot travel in model metres, before `step_scale`.
    pub stride: f32,
    /// Relaxed swing time; speed and trailing feet can shorten it to 80 ms.
    pub step_duration: f32,
    /// Base swing clearance in model metres, before `step_scale`.
    pub step_height: f32,
    /// Scales forward prediction, maximum foot travel and swing clearance.
    /// This changes the IK goal, not a joint angle or the knee bend direction.
    pub step_scale: f32,
}

impl Default for WalkingIkComponent {
    fn default() -> Self {
        Self {
            enabled: true,
            stride: 0.32,
            step_duration: 0.22,
            step_height: 0.07,
            step_scale: 1.3,
        }
    }
}

impl WalkingIkComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn from_avatar(avatar: &AvatarControlComponent) -> Self {
        Self {
            enabled: avatar.walking_enabled,
            stride: avatar.walking_stride,
            step_duration: avatar.walking_step_duration,
            step_height: avatar.walking_step_height,
            ..Self::default()
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        for (value, min, max, error) in [
            (
                self.stride,
                0.05,
                1.0,
                "walking stride must be model metres in [0.05,1.0]",
            ),
            (
                self.step_duration,
                0.08,
                1.0,
                "walking step duration must be seconds in [0.08,1.0]",
            ),
            (
                self.step_height,
                0.0,
                0.3,
                "walking step height must be model metres in [0,0.3]",
            ),
            (
                self.step_scale,
                0.1,
                2.0,
                "walking step scale must be in [0.1,2.0]",
            ),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(error);
            }
        }
        Ok(())
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_stride(mut self, metres: f32) -> Result<Self, String> {
        self.stride = metres;
        self.validate().map_err(str::to_owned)?;
        Ok(self)
    }

    pub fn with_step_duration(mut self, seconds: f32) -> Result<Self, String> {
        self.step_duration = seconds;
        self.validate().map_err(str::to_owned)?;
        Ok(self)
    }

    pub fn with_step_height(mut self, metres: f32) -> Result<Self, String> {
        self.step_height = metres;
        self.validate().map_err(str::to_owned)?;
        Ok(self)
    }

    pub fn with_step_scale(mut self, scale: f32) -> Result<Self, String> {
        self.step_scale = scale;
        self.validate().map_err(str::to_owned)?;
        Ok(self)
    }
}

impl Component for WalkingIkComponent {
    fn name(&self) -> &'static str {
        "walking_ik"
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
        ce("WalkingIK")
            .with_call("enabled", vec![b(self.enabled)])
            .with_call("stride", vec![num(self.stride as f64)])
            .with_call("step_duration", vec![num(self.step_duration as f64)])
            .with_call("step_height", vec![num(self.step_height as f64)])
            .with_call("step_scale", vec![num(self.step_scale as f64)])
    }
}
