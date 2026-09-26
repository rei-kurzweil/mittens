use super::{Component, ce_helpers::*};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// Settles the parent secondary-motion simulation at its imported rest pose.
/// `motion_threshold` is an anchor speed in metres per second; angular speed
/// contributes as motion at a 10 cm radius.
#[derive(Debug, Clone)]
pub struct ReturnToRestWhenStillComponent {
    pub motion_threshold: f32,
    pub still_for: f32,
}

impl Default for ReturnToRestWhenStillComponent {
    fn default() -> Self {
        Self {
            motion_threshold: 0.02,
            still_for: 0.4,
        }
    }
}

impl ReturnToRestWhenStillComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn motion_threshold(mut self, value: f32) -> Self {
        self.motion_threshold = value.max(0.0);
        self
    }

    pub fn still_for(mut self, value: f32) -> Self {
        self.still_for = value.max(0.0);
        self
    }
}

impl Component for ReturnToRestWhenStillComponent {
    fn name(&self) -> &'static str {
        "return_to_rest_when_still"
    }

    fn init(&mut self, emit: &mut dyn SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            IntentValue::RegisterSecondaryMotion {
                component_id: component,
            },
        );
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
        ce("ReturnToRestWhenStill")
            .with_call("motion_threshold", vec![num(self.motion_threshold as f64)])
            .with_call("still_for", vec![num(self.still_for as f64)])
    }
}
