//! Controller actions for the host-owned OpenXR session.
use crate::xr_host::{ApiResult, Report, Result};
use mittens_engine::engine::ecs::system::{
    XrGamepadState, XrHandGamepadState, XrInputState, external_xr_frame::ExternalXrFrame,
};
use openxr as xr;

pub(crate) struct Controllers {
    set: xr::ActionSet,
    aim: xr::Action<xr::Posef>,
    grip: xr::Action<xr::Posef>,
    sticks: xr::Action<xr::Vector2f>,
    trigger: xr::Action<f32>,
    squeeze: xr::Action<f32>,
    a: xr::Action<bool>,
    b: xr::Action<bool>,
    x: xr::Action<bool>,
    y: xr::Action<bool>,
    hands: [xr::Path; 2],
    aim_spaces: [xr::Space; 2],
    grip_spaces: [xr::Space; 2],
    previous: XrInputState,
    announced_aim: [bool; 2],
    announced_stick: [bool; 2],
}
impl Controllers {
    pub fn new(
        instance: &xr::Instance,
        session: &xr::Session<xr::Vulkan>,
        report: &mut Report,
    ) -> Result<Self> {
        let hands = [
            instance
                .string_to_path("/user/hand/left")
                .api("left hand path")?,
            instance
                .string_to_path("/user/hand/right")
                .api("right hand path")?,
        ];
        let set = instance
            .create_action_set("mittens_android", "Mittens", 0)
            .api("create controller action set")?;
        let aim = set
            .create_action::<xr::Posef>("aim", "Aim", &hands)
            .api("create aim action")?;
        let grip = set
            .create_action::<xr::Posef>("grip", "Grip", &hands)
            .api("create grip action")?;
        let sticks = set
            .create_action::<xr::Vector2f>("stick", "Thumbstick", &hands)
            .api("create stick action")?;
        let trigger = set
            .create_action::<f32>("trigger", "Trigger", &hands)
            .api("create trigger action")?;
        let squeeze = set
            .create_action::<f32>("squeeze", "Squeeze", &hands)
            .api("create squeeze action")?;
        let a = set
            .create_action::<bool>("a", "A", &hands)
            .api("create A action")?;
        let b = set
            .create_action::<bool>("b", "B", &hands)
            .api("create B action")?;
        let x = set
            .create_action::<bool>("x", "X", &hands)
            .api("create X action")?;
        let y = set
            .create_action::<bool>("y", "Y", &hands)
            .api("create Y action")?;
        let path = |s: &str| instance.string_to_path(s).api("controller binding path");
        let mut bindings = Vec::new();
        for side in ["left", "right"] {
            bindings.push(xr::Binding::new(
                &aim,
                path(&format!("/user/hand/{side}/input/aim/pose"))?,
            ));
            bindings.push(xr::Binding::new(
                &grip,
                path(&format!("/user/hand/{side}/input/grip/pose"))?,
            ));
            bindings.push(xr::Binding::new(
                &sticks,
                path(&format!("/user/hand/{side}/input/thumbstick"))?,
            ));
            bindings.push(xr::Binding::new(
                &trigger,
                path(&format!("/user/hand/{side}/input/trigger/value"))?,
            ));
            bindings.push(xr::Binding::new(
                &squeeze,
                path(&format!("/user/hand/{side}/input/squeeze/value"))?,
            ));
        }
        for (action, side, name) in [
            (&a, "right", "a"),
            (&b, "right", "b"),
            (&x, "left", "x"),
            (&y, "left", "y"),
        ] {
            bindings.push(xr::Binding::new(
                action,
                path(&format!("/user/hand/{side}/input/{name}/click"))?,
            ));
        }
        let profile = path("/interaction_profiles/htc/vive_focus3_controller")?;
        instance
            .suggest_interaction_profile_bindings(profile, &bindings)
            .api("suggest Focus 3 controller bindings")?;
        session
            .attach_action_sets(&[&set])
            .api("attach controller action set")?;
        let aim_spaces = [
            aim.create_space(session.clone(), hands[0], xr::Posef::IDENTITY)
                .api("left aim space")?,
            aim.create_space(session.clone(), hands[1], xr::Posef::IDENTITY)
                .api("right aim space")?,
        ];
        let grip_spaces = [
            grip.create_space(session.clone(), hands[0], xr::Posef::IDENTITY)
                .api("left grip space")?,
            grip.create_space(session.clone(), hands[1], xr::Posef::IDENTITY)
                .api("right grip space")?,
        ];
        report.line("Focus 3 controller actions attached");
        Ok(Self {
            set,
            aim,
            grip,
            sticks,
            trigger,
            squeeze,
            a,
            b,
            x,
            y,
            hands,
            aim_spaces,
            grip_spaces,
            previous: XrInputState::default(),
            announced_aim: [false; 2],
            announced_stick: [false; 2],
        })
    }

    pub fn sample(
        &mut self,
        session: &xr::Session<xr::Vulkan>,
        base: &xr::Space,
        time: xr::Time,
        views: [xr::View; 2],
    ) -> ExternalXrFrame {
        let active = session
            .sync_actions(&[xr::ActiveActionSet::new(&self.set)])
            .is_ok();
        let mut input = XrInputState::default();
        let mut gamepad = XrGamepadState {
            active,
            ..Default::default()
        };
        let mut aim = [None; 2];
        let mut grip = [None; 2];
        if active {
            for i in 0..2 {
                let hand = self.hands[i];
                let locate = |action: &xr::Action<xr::Posef>, space: &xr::Space| {
                    if !action.is_active(session, hand).unwrap_or(false) {
                        return None;
                    }
                    let location = space.locate(base, time).ok()?;
                    location
                        .location_flags
                        .contains(
                            xr::SpaceLocationFlags::POSITION_VALID
                                | xr::SpaceLocationFlags::ORIENTATION_VALID,
                        )
                        .then_some(location.pose)
                };
                aim[i] = locate(&self.aim, &self.aim_spaces[i]);
                grip[i] = locate(&self.grip, &self.grip_spaces[i]);
                if aim[i].is_some() && !self.announced_aim[i] {
                    log::info!(
                        "XR controller {} aim tracking active",
                        if i == 0 { "left" } else { "right" }
                    );
                    self.announced_aim[i] = true;
                }
                let scalar = |action: &xr::Action<f32>| {
                    action
                        .state(session, hand)
                        .ok()
                        .filter(|v| v.is_active)
                        .map(|v| v.current_state)
                };
                let boolean = |action: &xr::Action<bool>| {
                    action
                        .state(session, hand)
                        .ok()
                        .filter(|v| v.is_active)
                        .map(|v| (v.current_state, if v.current_state { 1.0 } else { 0.0 }))
                };
                let trigger = scalar(&self.trigger);
                let squeeze = scalar(&self.squeeze);
                let trigger_down = trigger.is_some_and(|v| v >= 0.5);
                let grip_down = squeeze.is_some_and(|v| v >= 0.5);
                input.trigger_down[i] = trigger_down;
                input.trigger_pressed[i] = trigger_down && !self.previous.trigger_down[i];
                input.trigger_released[i] = !trigger_down && self.previous.trigger_down[i];
                input.grip_down[i] = grip_down;
                input.grip_pressed[i] = grip_down && !self.previous.grip_down[i];
                input.grip_released[i] = !grip_down && self.previous.grip_down[i];
                let stick = self
                    .sticks
                    .state(session, hand)
                    .ok()
                    .filter(|v| v.is_active)
                    .map(|v| [v.current_state.x, v.current_state.y]);
                if stick.is_some() && !self.announced_stick[i] {
                    log::info!(
                        "XR controller {} thumbstick active",
                        if i == 0 { "left" } else { "right" }
                    );
                    self.announced_stick[i] = true;
                }
                gamepad.hands[i] = XrHandGamepadState {
                    thumbstick: stick,
                    trigger_value: trigger,
                    trigger_pressed: trigger.map(|v| (v >= 0.5, v)),
                    grip_value: squeeze,
                    grip_pressed: squeeze.map(|v| (v >= 0.5, v)),
                    button_a: boolean(&self.a),
                    button_b: boolean(&self.b),
                    button_x: boolean(&self.x),
                    button_y: boolean(&self.y),
                };
            }
        } else {
            input.trigger_released = self.previous.trigger_down;
            input.grip_released = self.previous.grip_down;
        }
        let orientation = views[0].pose.orientation;
        gamepad.head_pose_rotation =
            Some([orientation.x, orientation.y, orientation.z, orientation.w]);
        self.previous = input;
        ExternalXrFrame {
            views: Some(views),
            aim,
            grip,
            input,
            gamepad,
        }
    }
}
