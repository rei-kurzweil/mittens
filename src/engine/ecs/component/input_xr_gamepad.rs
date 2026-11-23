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

/// What the locomotion stick's X axis does; Y always moves forward and back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum XrStickMode {
    /// X strafes left and right.
    #[default]
    Strafe,
    /// X smoothly turns the rig about the headset.
    Turn,
}

impl XrStickMode {
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "strafe" => Some(Self::Strafe),
            "turn" => Some(Self::Turn),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Strafe => "strafe",
            Self::Turn => "turn",
        }
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
    pub stick_mode: XrStickMode,
    /// Turn-mode yaw rate at full stick deflection, in radians per second.
    pub turn_speed: f32,
    /// Opt-in B double-tap flight. Runtime flight state is never serialized.
    pub enable_flying: bool,
    pub(crate) flying: bool,
    pub deadzone: f32,
    pub component_id: Option<ComponentId>,
}

const DEFAULT_TURN_SPEED: f32 = 2.0;

impl InputXRGamepadComponent {
    pub fn new() -> Self {
        Self {
            enabled: true,
            hand: XrHandPreference::Default,
            locomotion: true,
            speed: 3.0,
            stick_mode: XrStickMode::Strafe,
            turn_speed: DEFAULT_TURN_SPEED,
            enable_flying: false,
            flying: false,
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

    pub fn enable_flying(mut self, enable: bool) -> Self {
        self.enable_flying = enable;
        self
    }

    pub fn speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    pub fn stick_mode(mut self, mode: XrStickMode) -> Self {
        self.stick_mode = mode;
        self
    }

    pub fn turn_speed(mut self, turn_speed: f32) -> Self {
        self.turn_speed = turn_speed;
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
                component_id: component,
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
                component_id: component,
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;

        let hand = match self.hand {
            XrHandPreference::Default => "default",
            XrHandPreference::Left => "left",
            XrHandPreference::Right => "right",
            XrHandPreference::Either => "either",
        };

        let mut ce = ce_call("InputXRGamepad", "new", vec![])
            .with_call("enabled", vec![b(self.enabled)])
            .with_call("hand", vec![s(hand)]);
        if !self.locomotion {
            ce = ce.with_call("locomotion", vec![b(false)]);
        }
        if self.enable_flying {
            ce = ce.with_call("enable_flying", vec![b(true)]);
        }
        if (self.speed - 3.0).abs() > f32::EPSILON {
            ce = ce.with_call("speed", vec![num(self.speed as f64)]);
        }
        if self.stick_mode != XrStickMode::Strafe {
            ce = ce.with_call("stick_mode", vec![s(self.stick_mode.as_str())]);
        }
        if (self.turn_speed - DEFAULT_TURN_SPEED).abs() > f32::EPSILON {
            ce = ce.with_call("turn_speed", vec![num(self.turn_speed as f64)]);
        }
        if (self.deadzone - 0.2).abs() > f32::EPSILON {
            ce = ce.with_call("deadzone", vec![num(self.deadzone as f64)]);
        }
        ce
    }
}
