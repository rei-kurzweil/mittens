use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrHandPreference {
    Default,
    Left,
    Right,
    Either,
}

impl Default for XrHandPreference {
    fn default() -> Self {
        Self::Default
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XrAxisControl {
    LeftStick,
    RightStick,
    LeftTrigger,
    RightTrigger,
    LeftGrip,
    RightGrip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XrButtonControl {
    LeftTrigger,
    RightTrigger,
    LeftGrip,
    RightGrip,
    ButtonA,
    ButtonB,
    ButtonX,
    ButtonY,
}

#[derive(Debug, Clone)]
pub struct InputXRGamepadComponent {
    pub enabled: bool,
    pub hand: XrHandPreference,
    pub locomotion: bool,
    pub speed: f32,
    pub deadzone: f32,
    pub component_id: Option<ComponentId>,
}

impl InputXRGamepadComponent {
    pub fn new() -> Self {
        Self {
            enabled: true,
            hand: XrHandPreference::Default,
            locomotion: true,
            speed: 1.5,
            deadzone: 0.2,
            component_id: None,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn hand(mut self, hand: XrHandPreference) -> Self {
        self.hand = hand;
        self
    }

    pub fn locomotion(mut self) -> Self {
        self.locomotion = true;
        self
    }

    pub fn locomotion_enabled(mut self, enabled: bool) -> Self {
        self.locomotion = enabled;
        self
    }

    pub fn speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    pub fn deadzone(mut self, deadzone: f32) -> Self {
        self.deadzone = deadzone;
        self
    }
}

impl Default for InputXRGamepadComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for InputXRGamepadComponent {
    fn name(&self) -> &'static str {
        "input_vr_gamepad"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component_id = Some(component);
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        self.component_id = Some(component);
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterInputXrGamepad {
                component_ids: vec![component],
            },
        );
    }

    fn cleanup(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: ComponentId,
    ) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RemoveInputXrGamepad {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;

        let hand = match self.hand {
            XrHandPreference::Default => "default",
            XrHandPreference::Left => "left",
            XrHandPreference::Right => "right",
            XrHandPreference::Either => "either",
        };

        let mut ce = ce_call("InputVRGamepad", "new", vec![])
            .with_call("enabled", vec![b(self.enabled)])
            .with_call("hand", vec![s(hand)]);
        if !self.locomotion {
            ce = ce.with_call("locomotion", vec![b(false)]);
        }
        if (self.speed - 1.5).abs() > f32::EPSILON {
            ce = ce.with_call("speed", vec![num(self.speed as f64)]);
        }
        if (self.deadzone - 0.2).abs() > f32::EPSILON {
            ce = ce.with_call("deadzone", vec![num(self.deadzone as f64)]);
        }
        ce
    }
}
