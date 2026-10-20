use super::{Component, ce_helpers::*};
use crate::engine::ecs::system::hand_tracking::HandTrackingProvider;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// Opt-in rotation ownership for the fingers in the owning GLTF's humanoid map.
#[derive(Debug, Clone, PartialEq)]
pub struct HandTrackingComponent {
    pub provider: Option<HandTrackingProvider>,
    pub timeout_seconds: f32,
    pub release_seconds: f32,
}

impl Default for HandTrackingComponent {
    fn default() -> Self {
        Self {
            provider: None,
            timeout_seconds: 0.15,
            release_seconds: 0.15,
        }
    }
}

impl HandTrackingComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn source(mut self, source: &str) -> Result<Self, String> {
        self.provider = match source {
            "auto" => None,
            "openxr" => Some(HandTrackingProvider::OpenXr),
            "mediapipe" => Some(HandTrackingProvider::MediaPipe),
            _ => return Err(format!("unknown hand tracking source '{source}'")),
        };
        Ok(self)
    }

    pub fn timeout(mut self, seconds: f32) -> Result<Self, String> {
        if !seconds.is_finite() || seconds <= 0.0 {
            return Err("hand timeout must be positive and finite".into());
        }
        self.timeout_seconds = seconds;
        Ok(self)
    }

    pub fn release_duration(mut self, seconds: f32) -> Result<Self, String> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err("hand release duration must be nonnegative and finite".into());
        }
        self.release_seconds = seconds;
        Ok(self)
    }
}

impl Component for HandTrackingComponent {
    fn name(&self) -> &'static str {
        "hand_tracking"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn init(&mut self, emit: &mut dyn SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            IntentValue::RegisterHandTracking {
                component_id: component,
            },
        );
    }
    fn cleanup(&mut self, emit: &mut dyn SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            IntentValue::UnregisterHandTracking {
                component_id: component,
            },
        );
    }
    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        let source = match self.provider {
            None => "auto",
            Some(HandTrackingProvider::OpenXr) => "openxr",
            Some(HandTrackingProvider::MediaPipe) => "mediapipe",
        };
        ce_call("HandTracking", "new", vec![])
            .with_call("source", vec![s(source)])
            .with_call("timeout", vec![num(self.timeout_seconds as f64)])
            .with_call("release_duration", vec![num(self.release_seconds as f64)])
    }
}
