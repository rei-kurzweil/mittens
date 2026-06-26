/// Per-frame trigger activation state for XR controllers.
///
/// Mirrors the edge-triggered / level-triggered structure of `InputState` for mouse buttons,
/// but for the XR select action. Index 0 = left hand, 1 = right hand.
///
/// Reset to default each frame when XR is not running.
#[derive(Default, Debug, Clone, Copy)]
pub struct XrInputState {
    pub trigger_pressed: [bool; 2],
    pub trigger_down: [bool; 2],
    pub trigger_released: [bool; 2],
}

#[derive(Default, Debug, Clone, Copy)]
pub struct XrHandGamepadState {
    pub thumbstick: Option<[f32; 2]>,
    pub trigger_value: Option<f32>,
    pub trigger_pressed: Option<(bool, f32)>,
    pub grip_value: Option<f32>,
    pub grip_pressed: Option<(bool, f32)>,
    pub button_a: Option<(bool, f32)>,
    pub button_b: Option<(bool, f32)>,
    pub button_x: Option<(bool, f32)>,
    pub button_y: Option<(bool, f32)>,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct XrGamepadState {
    pub active: bool,
    pub hands: [XrHandGamepadState; 2],
    pub head_pose_rotation: Option<[f32; 4]>,
}
