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

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("enabled".to_string(), serde_json::json!(self.enabled));
        map.insert(
            "duration_beats".to_string(),
            serde_json::json!(self.duration_beats),
        );
        map.insert("easing".to_string(), serde_json::json!(self.easing.as_str()));
        map.insert(
            "capture_from_current".to_string(),
            serde_json::json!(self.capture_from_current),
        );
        map.insert("replace".to_string(), serde_json::json!(self.replace.as_str()));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(enabled) = data.get("enabled") {
            self.enabled = serde_json::from_value(enabled.clone())
                .map_err(|e| format!("Failed to decode enabled: {}", e))?;
        }
        if let Some(duration_beats) = data.get("duration_beats") {
            let parsed: f64 = serde_json::from_value(duration_beats.clone())
                .map_err(|e| format!("Failed to decode duration_beats: {}", e))?;
            self.duration_beats = if parsed.is_finite() { parsed.max(0.0) } else { 0.0 };
        }
        if let Some(easing) = data.get("easing").and_then(|v| v.as_str()) {
            self.easing = TransitionEasing::parse(easing)?;
        }
        if let Some(capture_from_current) = data.get("capture_from_current") {
            self.capture_from_current = serde_json::from_value(capture_from_current.clone())
                .map_err(|e| format!("Failed to decode capture_from_current: {}", e))?;
        }
        if let Some(replace) = data.get("replace").and_then(|v| v.as_str()) {
            self.replace = TransitionReplacePolicy::parse(replace)?;
        }
        Ok(())
    }
}