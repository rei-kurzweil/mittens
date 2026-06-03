use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionEasing {
    Step,
    Linear,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInOutSine,
}

impl TransitionEasing {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Step => "step",
            Self::Linear => "linear",
            Self::EaseInQuad => "ease_in_quad",
            Self::EaseOutQuad => "ease_out_quad",
            Self::EaseInOutQuad => "ease_in_out_quad",
            Self::EaseInCubic => "ease_in_cubic",
            Self::EaseOutCubic => "ease_out_cubic",
            Self::EaseInOutCubic => "ease_in_out_cubic",
            Self::EaseInOutSine => "ease_in_out_sine",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "step" => Ok(Self::Step),
            "linear" => Ok(Self::Linear),
            "ease_in_quad" => Ok(Self::EaseInQuad),
            "ease_out_quad" => Ok(Self::EaseOutQuad),
            "ease_in_out_quad" => Ok(Self::EaseInOutQuad),
            "ease_in_cubic" => Ok(Self::EaseInCubic),
            "ease_out_cubic" => Ok(Self::EaseOutCubic),
            "ease_in_out_cubic" => Ok(Self::EaseInOutCubic),
            "ease_in_out_sine" => Ok(Self::EaseInOutSine),
            other => Err(format!("Unknown transition easing: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionReplacePolicy {
    ReplaceSameTarget,
    AllowParallel,
}

impl TransitionReplacePolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReplaceSameTarget => "replace_same_target",
            Self::AllowParallel => "allow_parallel",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "replace_same_target" => Ok(Self::ReplaceSameTarget),
            "allow_parallel" => Ok(Self::AllowParallel),
            other => Err(format!("Unknown transition replace policy: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionComponent {
    pub enabled: bool,
    pub duration_beats: f64,
    pub easing: TransitionEasing,
    pub capture_from_current: bool,
    pub replace: TransitionReplacePolicy,
}

impl TransitionComponent {
    pub fn new() -> Self {
        Self {
            enabled: true,
            duration_beats: 0.0,
            easing: TransitionEasing::Linear,
            capture_from_current: true,
            replace: TransitionReplacePolicy::ReplaceSameTarget,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn on(self) -> Self {
        self.enabled(true)
    }

    pub fn off(self) -> Self {
        self.enabled(false)
    }

    pub fn with_duration_beats(mut self, duration_beats: f64) -> Self {
        self.duration_beats = if duration_beats.is_finite() {
            duration_beats.max(0.0)
        } else {
            0.0
        };
        self
    }

    pub fn with_capture_from_current(mut self, capture_from_current: bool) -> Self {
        self.capture_from_current = capture_from_current;
        self
    }

    pub fn with_easing(mut self, easing: TransitionEasing) -> Self {
        self.easing = easing;
        self
    }

    pub fn with_replace(mut self, replace: TransitionReplacePolicy) -> Self {
        self.replace = replace;
        self
    }
}

impl Default for TransitionComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for TransitionComponent {
    fn name(&self) -> &'static str {
        "transition"
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
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce("Transition")
            .with_call("enabled", vec![b(self.enabled)])
            .with_call("duration_beats", vec![num(self.duration_beats)])
            .with_call(self.easing.as_str(), vec![])
            .with_call("capture_from_current", vec![b(self.capture_from_current)])
            .with_call(self.replace.as_str(), vec![])
    }
}
