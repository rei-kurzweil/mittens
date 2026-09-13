use super::Component;
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone)]
pub struct KeyframeComponent {
    /// When this keyframe should fire, in beats.
    pub beat: f64,
    /// Opaque callback retained by a `meow-meow-script` session. This carries
    /// no executable state into the ECS.
    pub session_callback: Option<meow_meow_script::SessionCallbackRef>,
    /// Session-computed classification for `session_callback`.
    pub effect_profile: meow_meow_script::KeyframeEffectProfile,

    component: Option<ComponentId>,
}

impl KeyframeComponent {
    pub fn new(beat: f64) -> Self {
        Self {
            beat,
            session_callback: None,
            effect_profile: meow_meow_script::KeyframeEffectProfile::None,
            component: None,
        }
    }

    pub fn new_with_session_callback(
        beat: f64,
        callback: meow_meow_script::SessionCallbackRef,
        effect_profile: meow_meow_script::KeyframeEffectProfile,
    ) -> Self {
        Self {
            beat,
            session_callback: Some(callback),
            effect_profile,
            component: None,
        }
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }
}

impl Component for KeyframeComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "keyframe"
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterKeyframe {
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
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call("Keyframe", "at", vec![num(self.beat)])
    }
}
