use super::Component;
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationState {
    Playing,
    Looping,
    Paused,
}

#[derive(Debug, Clone, Copy)]
pub struct AnimationComponent {
    pub state: AnimationState,

    component: Option<ComponentId>,
}

impl AnimationComponent {
    pub fn new() -> Self {
        Self {
            state: AnimationState::Looping,
            component: None,
        }
    }

    pub fn with_state(mut self, state: AnimationState) -> Self {
        self.state = state;
        self
    }

    /// Backward-compatible helper for older callers.
    pub fn with_playing(self, playing: bool) -> Self {
        self.with_state(if playing {
            AnimationState::Looping
        } else {
            AnimationState::Paused
        })
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }
}

impl Default for AnimationComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for AnimationComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "animation"
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterAnimation {
                component_ids: vec![component],
            },
        );
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(&self, _world: &crate::engine::ecs::World) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = match self.state {
            AnimationState::Playing => "playing",
            AnimationState::Looping => "looping",
            AnimationState::Paused => "paused",
        };
        ce_call("Animation", ctor, vec![])
    }
}
