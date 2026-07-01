use super::Component;
use crate::engine::ecs::ComponentId;
use crate::meow_meow::object::CapturedBlock;

#[derive(Debug, Clone)]
pub struct KeyframeComponent {
    /// When this keyframe should fire, in beats.
    pub beat: f64,
    pub callback: Option<CapturedBlock>,

    component: Option<ComponentId>,
}

impl KeyframeComponent {
    pub fn new(beat: f64) -> Self {
        Self {
            beat,
            callback: None,
            component: None,
        }
    }

    pub fn new_with_callback(beat: f64, callback: CapturedBlock) -> Self {
        Self {
            beat,
            callback: Some(callback),
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

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut ce = ce_call("Keyframe", "at", vec![num(self.beat)]);
        if let Some(callback) = &self.callback {
            ce.body = callback.body.clone();
        }
        ce
    }
}
