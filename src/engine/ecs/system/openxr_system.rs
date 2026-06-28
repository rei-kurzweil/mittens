use crate::engine::ecs::component::CameraXRComponent;
use crate::engine::ecs::component::{
    ControllerHand, ControllerPoseKind, InputVRComponent, VRHandComponent, VrComponent,
};
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::system::vr_backend::{VrBackend, VrBackendKind};
use crate::engine::ecs::system::vr_types::{XrGamepadState, XrHandGamepadState, XrInputState};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::CameraData;
use crate::engine::graphics::VisualWorld;
use crate::engine::graphics::VulkanoRenderer;
use crate::engine::graphics::XRSwapchain;
use crate::engine::graphics::XrVulkanGraphics;
use crate::engine::graphics::xr_renderer;
use crate::engine::user_input::InputState;
use crate::utils::math;

use ash::vk::Handle as _;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::OnceLock;
use std::time::Instant;

fn openxr_debug_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CAT_OPENXR_DEBUG")
            .ok()
            .map(|s| {
                let s = s.trim().to_ascii_lowercase();
                s == "1" || s == "true" || s == "on" || s == "yes"
            })
            .unwrap_or(false)
    })
}

fn log_xr_gamepad_changes(prev: XrGamepadState, next: XrGamepadState) {
    if !openxr_debug_enabled() {
        return;
    }
    if prev.active != next.active {
        eprintln!(
            "[OpenXR][gamepad] active -> {}",
            if next.active { "true" } else { "false" }
        );
    }
    for (index, hand_name) in ["left", "right"].into_iter().enumerate() {
        let before = prev.hands[index];
        let after = next.hands[index];
        if before.thumbstick != after.thumbstick {
            eprintln!(
                "[OpenXR][gamepad] {hand_name} thumbstick -> {:?}",
                after.thumbstick
            );
        }
        if before.trigger_value != after.trigger_value {
            eprintln!(
                "[OpenXR][gamepad] {hand_name} trigger_value -> {:?}",
                after.trigger_value
            );
        }
        if before.trigger_pressed != after.trigger_pressed {
            eprintln!(
                "[OpenXR][gamepad] {hand_name} trigger_pressed -> {:?}",
                after.trigger_pressed
            );
        }
        if before.grip_value != after.grip_value {
            eprintln!(
                "[OpenXR][gamepad] {hand_name} grip_value -> {:?}",
                after.grip_value
            );
        }
        if before.grip_pressed != after.grip_pressed {
            eprintln!(
                "[OpenXR][gamepad] {hand_name} grip_pressed -> {:?}",
                after.grip_pressed
            );
        }
        if before.button_x != after.button_x {
            eprintln!(
                "[OpenXR][gamepad] {hand_name} button_x -> {:?}",
                after.button_x
            );
        }
        if before.button_y != after.button_y {
            eprintln!(
                "[OpenXR][gamepad] {hand_name} button_y -> {:?}",
                after.button_y
            );
        }
        if before.button_a != after.button_a {
            eprintln!(
                "[OpenXR][gamepad] {hand_name} button_a -> {:?}",
                after.button_a
            );
        }
        if before.button_b != after.button_b {
            eprintln!(
                "[OpenXR][gamepad] {hand_name} button_b -> {:?}",
                after.button_b
            );
        }
    }
}

fn log_active_interaction_profile(
    instance: &openxr::Instance,
    session: &openxr::Session<openxr::Vulkan>,
    user_path: openxr::Path,
    user_path_str: &str,
    last_logged: &mut Option<openxr::Path>,
) {
    let Ok(profile) = session.current_interaction_profile(user_path) else {
        return;
    };
    if *last_logged == Some(profile) {
        return;
    }
    *last_logged = Some(profile);
    if profile == openxr::Path::NULL {
        eprintln!("[OpenXR][profile] {user_path_str}: no active interaction profile");
        return;
    }
    match instance.path_to_string(profile) {
        Ok(profile_str) => {
            eprintln!("[OpenXR][profile] {user_path_str}: active profile = {profile_str}");
        }
        Err(err) => {
            eprintln!(
                "[OpenXR][profile] {user_path_str}: active profile path {:?} (path_to_string failed: {:?})",
                profile, err
            );
        }
    }
}

fn profile_string_or_none(
    instance: &openxr::Instance,
    profile: Option<openxr::Path>,
) -> String {
    match profile {
        Some(profile) if profile == openxr::Path::NULL => "none".to_string(),
        Some(profile) => instance
            .path_to_string(profile)
            .unwrap_or_else(|_| format!("{profile:?}")),
        None => "unknown".to_string(),
    }
}

fn scalar_stick_value(
    x: Option<openxr::ActionState<f32>>,
    y: Option<openxr::ActionState<f32>>,
) -> Option<[f32; 2]> {
    match (x, y) {
        (Some(x), Some(y)) if x.is_active || y.is_active => Some([x.current_state, y.current_state]),
        _ => None,
    }
}

fn scalar_stick_state(
    x: Option<openxr::ActionState<f32>>,
    y: Option<openxr::ActionState<f32>>,
) -> Option<(bool, [f32; 2])> {
    match (x, y) {
        (Some(x), Some(y)) => Some((x.is_active || y.is_active, [x.current_state, y.current_state])),
        _ => None,
    }
}

fn append_action_bindings<'a>(
    instance: &openxr::Instance,
    suggested: &mut Vec<(&'static str, &'static str, openxr::Binding<'a>)>,
    spec: BindingSpec,
    left_stick_x: &'a openxr::Action<f32>,
    left_stick_y: &'a openxr::Action<f32>,
    right_stick_x: &'a openxr::Action<f32>,
    right_stick_y: &'a openxr::Action<f32>,
    trigger_value: &'a openxr::Action<f32>,
    trigger_click: &'a openxr::Action<bool>,
    grip_value: &'a openxr::Action<f32>,
    grip_click: &'a openxr::Action<bool>,
    button_a: &'a openxr::Action<bool>,
    button_b: &'a openxr::Action<bool>,
    button_x: &'a openxr::Action<bool>,
    button_y: &'a openxr::Action<bool>,
) {
    for &path_str in spec.paths {
        let Ok(path) = instance.string_to_path(path_str) else {
            continue;
        };
        match spec.label {
            "left_stick_x" => suggested.push((spec.label, path_str, openxr::Binding::new(left_stick_x, path))),
            "left_stick_y" => suggested.push((spec.label, path_str, openxr::Binding::new(left_stick_y, path))),
            "right_stick_x" => suggested.push((spec.label, path_str, openxr::Binding::new(right_stick_x, path))),
            "right_stick_y" => suggested.push((spec.label, path_str, openxr::Binding::new(right_stick_y, path))),
            "trigger_value" => suggested.push((spec.label, path_str, openxr::Binding::new(trigger_value, path))),
            "trigger_click" => suggested.push((spec.label, path_str, openxr::Binding::new(trigger_click, path))),
            "grip_value" => suggested.push((spec.label, path_str, openxr::Binding::new(grip_value, path))),
            "grip_click" => suggested.push((spec.label, path_str, openxr::Binding::new(grip_click, path))),
            "a" => suggested.push((spec.label, path_str, openxr::Binding::new(button_a, path))),
            "b" => suggested.push((spec.label, path_str, openxr::Binding::new(button_b, path))),
            "x" => suggested.push((spec.label, path_str, openxr::Binding::new(button_x, path))),
            "y" => suggested.push((spec.label, path_str, openxr::Binding::new(button_y, path))),
            _ => {}
        }
    }
}

fn suggest_binding_best_effort(
    instance: &openxr::Instance,
    profile: openxr::Path,
    profile_name: &str,
    binding_label: &str,
    binding_path: &str,
    binding: openxr::Binding<'_>,
) -> Result<(), String> {
    match instance.suggest_interaction_profile_bindings(profile, &[binding]) {
        Ok(()) => Ok(()),
        Err(openxr::sys::Result::ERROR_PATH_UNSUPPORTED) => {
            eprintln!(
                "[OpenXR] ignoring unsupported binding profile={} label={} path={}",
                profile_name, binding_label, binding_path
            );
            Ok(())
        }
        Err(err) => Err(format!(
            "suggest_interaction_profile_bindings({}, {} -> {}): {err:?}",
            profile_name, binding_label, binding_path
        )),
    }
}

fn log_profile_binding_dump(spec: &ProfileBindingSpec) {
    if !openxr_debug_enabled() {
        return;
    }
    eprintln!("[OpenXR][binding_dump] profile {}", spec.profile);
    eprintln!(
        "[OpenXR][binding_dump]   select left={:?} right={:?}",
        spec.select_left, spec.select_right
    );
    for spec in spec.left_specs {
        eprintln!(
            "[OpenXR][binding_dump]   left {:<13} -> {}",
            spec.label,
            spec.paths.join(", ")
        );
    }
    for spec in spec.right_specs {
        eprintln!(
            "[OpenXR][binding_dump]   right {:<12} -> {}",
            spec.label,
            spec.paths.join(", ")
        );
    }
}

pub struct OpenXRSystem {
    state: Option<OpenXRState>,
    last_init_error: Option<String>,
    vulkan_graphics: Option<XrVulkanGraphics>,
    preferred_swapchain_format: Option<u32>,

    // Best-effort XR frame timing diagnostics.
    last_render_instant: Option<Instant>,
    last_render_dt_sec: Option<f32>,

    input_xr_components: HashSet<ComponentId>,
    controller_components: HashSet<ComponentId>,
    controller_pose_source_last_logged: HashMap<ComponentId, &'static str>,

    xr_input_state: XrInputState,
    xr_gamepad_state: XrGamepadState,
    xr_gamepad_state_last_logged: XrGamepadState,
}

struct OpenXRState {
    #[allow(dead_code)]
    entry: openxr::Entry,
    #[allow(dead_code)]
    instance: openxr::Instance,
    #[allow(dead_code)]
    system: openxr::SystemId,
    events: openxr::EventDataBuffer,

    session: Option<OpenXRSessionState>,
    view_type: openxr::ViewConfigurationType,
    blend_mode: openxr::EnvironmentBlendMode,
}

struct OpenXRSessionState {
    session: openxr::Session<openxr::Vulkan>,
    frame_waiter: openxr::FrameWaiter,
    frame_stream: openxr::FrameStream<openxr::Vulkan>,
    reference_space: openxr::Space,
    running: bool,
    current_state: openxr::SessionState,

    xr_swapchain: XRSwapchain,

    swapchain_image_initialized: Vec<bool>,

    did_log_format_mismatch: bool,

    vk_device: ash::Device,
    vk_queue: ash::vk::Queue,

    #[allow(dead_code)]
    vk_command_pool: ash::vk::CommandPool,
    vk_command_buffer: ash::vk::CommandBuffer,

    hand_tracking: Option<HandTrackingState>,
    hand_root_pose_cache: HandRootPoseCache,
    hand_rotation_debug: HandRotationDebugState,
    last_hand_debug_snapshot: Option<HandDebugSnapshot>,
    head_pose_cache: Option<openxr::Posef>,
    controller_input: Option<ControllerInput>,
    controller_pose_cache: ControllerPoseCache,
}

#[derive(Debug, Default, Clone, Copy)]
struct ControllerPoseCache {
    left_aim: Option<openxr::Posef>,
    right_aim: Option<openxr::Posef>,
    left_grip: Option<openxr::Posef>,
    right_grip: Option<openxr::Posef>,
}

struct HandTrackingState {
    left: openxr::HandTracker,
    right: openxr::HandTracker,
}

#[derive(Debug, Default, Clone, Copy)]
struct HandRootPoseCache {
    left_root: Option<openxr::Posef>,
    right_root: Option<openxr::Posef>,
    left_root_joint: Option<openxr::HandJointEXT>,
    right_root_joint: Option<openxr::HandJointEXT>,
}

#[derive(Debug, Default)]
struct HandRotationDebugState {
    left: RollingAngleWindow,
    right: RollingAngleWindow,
}

#[derive(Debug, Default)]
struct RollingAngleWindow {
    previous_quat_xyzw: Option<[f32; 4]>,
    step_deg: VecDeque<f32>,
    sample_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HandDebugState {
    root_joint: Option<openxr::HandJointEXT>,
    wrist_palm_delta_deg: Option<u16>,
    wrist_step_deg: Option<u16>,
    wrist_step_spike: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HandDebugSnapshot {
    left: HandDebugState,
    right: HandDebugState,
}

#[derive(Debug, Clone, PartialEq)]
struct ControllerDebugSnapshot {
    left_profile: String,
    right_profile: String,
    left_aim_valid: bool,
    right_aim_valid: bool,
    left_grip_valid: bool,
    right_grip_valid: bool,
    left_select: Option<(bool, bool)>,
    right_select: Option<(bool, bool)>,
    left_thumbstick: Option<(bool, [f32; 2])>,
    right_thumbstick: Option<(bool, [f32; 2])>,
    left_trigger_value: Option<(bool, f32)>,
    right_trigger_value: Option<(bool, f32)>,
    left_trigger_click: Option<(bool, bool)>,
    right_trigger_click: Option<(bool, bool)>,
    left_grip_value: Option<(bool, f32)>,
    right_grip_value: Option<(bool, f32)>,
    left_grip_click: Option<(bool, bool)>,
    right_grip_click: Option<(bool, bool)>,
    left_x: Option<(bool, bool)>,
    left_y: Option<(bool, bool)>,
    right_a: Option<(bool, bool)>,
    right_b: Option<(bool, bool)>,
}

#[derive(Debug, Clone, Copy)]
struct BindingSpec {
    label: &'static str,
    paths: &'static [&'static str],
}

#[derive(Debug, Clone, Copy)]
struct ProfileBindingSpec {
    profile: &'static str,
    select_left: Option<&'static str>,
    select_right: Option<&'static str>,
    left_specs: &'static [BindingSpec],
    right_specs: &'static [BindingSpec],
}

#[allow(dead_code)]
struct ControllerInput {
    action_set: openxr::ActionSet,
    aim_pose: openxr::Action<openxr::Posef>,
    grip_pose: openxr::Action<openxr::Posef>,
    select: openxr::Action<bool>,
    left_stick_x: openxr::Action<f32>,
    left_stick_y: openxr::Action<f32>,
    right_stick_x: openxr::Action<f32>,
    right_stick_y: openxr::Action<f32>,
    trigger_value: openxr::Action<f32>,
    trigger_click: openxr::Action<bool>,
    grip_value: openxr::Action<f32>,
    grip_click: openxr::Action<bool>,
    button_a: openxr::Action<bool>,
    button_b: openxr::Action<bool>,
    button_x: openxr::Action<bool>,
    button_y: openxr::Action<bool>,

    left: openxr::Path,
    right: openxr::Path,

    left_aim_space: openxr::Space,
    right_aim_space: openxr::Space,
    left_grip_space: openxr::Space,
    right_grip_space: openxr::Space,

    profile_poll_counter: u32,
    last_logged_left_profile: Option<openxr::Path>,
    last_logged_right_profile: Option<openxr::Path>,
    last_debug_snapshot: Option<ControllerDebugSnapshot>,
}

const PROFILE_SIMPLE_CONTROLLER: &str = "/interaction_profiles/khr/simple_controller";
const PROFILE_OCULUS_TOUCH: &str = "/interaction_profiles/oculus/touch_controller";
const PROFILE_HTC_VIVE: &str = "/interaction_profiles/htc/vive_controller";
const PROFILE_HTC_VIVE_FOCUS3: &str = "/interaction_profiles/htc/vive_focus3_controller";
const PROFILE_VALVE_INDEX: &str = "/interaction_profiles/valve/index_controller";
const PROFILE_MICROSOFT_MOTION: &str = "/interaction_profiles/microsoft/motion_controller";
const PROFILE_EXT_HAND_INTERACTION: &str = "/interaction_profiles/ext/hand_interaction_ext";

const SIMPLE_CONTROLLER_LEFT_SPECS: &[BindingSpec] = &[];
const SIMPLE_CONTROLLER_RIGHT_SPECS: &[BindingSpec] = &[];

const OCULUS_TOUCH_LEFT_SPECS: &[BindingSpec] = &[
    BindingSpec { label: "left_stick_x", paths: &["/user/hand/left/input/thumbstick/x", "/user/hand/left/input/joystick/x"] },
    BindingSpec { label: "left_stick_y", paths: &["/user/hand/left/input/thumbstick/y", "/user/hand/left/input/joystick/y"] },
    BindingSpec { label: "trigger_value", paths: &["/user/hand/left/input/trigger/value"] },
    BindingSpec { label: "trigger_click", paths: &["/user/hand/left/input/trigger/click"] },
    BindingSpec { label: "grip_value", paths: &["/user/hand/left/input/squeeze/value", "/user/hand/left/input/grip/value"] },
    BindingSpec { label: "grip_click", paths: &["/user/hand/left/input/squeeze/click", "/user/hand/left/input/grip/click"] },
    BindingSpec { label: "x", paths: &["/user/hand/left/input/x/click"] },
    BindingSpec { label: "y", paths: &["/user/hand/left/input/y/click"] },
];
const OCULUS_TOUCH_RIGHT_SPECS: &[BindingSpec] = &[
    BindingSpec { label: "right_stick_x", paths: &["/user/hand/right/input/thumbstick/x", "/user/hand/right/input/joystick/x"] },
    BindingSpec { label: "right_stick_y", paths: &["/user/hand/right/input/thumbstick/y", "/user/hand/right/input/joystick/y"] },
    BindingSpec { label: "trigger_value", paths: &["/user/hand/right/input/trigger/value"] },
    BindingSpec { label: "trigger_click", paths: &["/user/hand/right/input/trigger/click"] },
    BindingSpec { label: "grip_value", paths: &["/user/hand/right/input/squeeze/value", "/user/hand/right/input/grip/value"] },
    BindingSpec { label: "grip_click", paths: &["/user/hand/right/input/squeeze/click", "/user/hand/right/input/grip/click"] },
    BindingSpec { label: "a", paths: &["/user/hand/right/input/a/click"] },
    BindingSpec { label: "b", paths: &["/user/hand/right/input/b/click"] },
];

const VALVE_INDEX_LEFT_SPECS: &[BindingSpec] = &[
    BindingSpec { label: "left_stick_x", paths: &["/user/hand/left/input/thumbstick/x", "/user/hand/left/input/joystick/x"] },
    BindingSpec { label: "left_stick_y", paths: &["/user/hand/left/input/thumbstick/y", "/user/hand/left/input/joystick/y"] },
    BindingSpec { label: "trigger_value", paths: &["/user/hand/left/input/trigger/value"] },
    BindingSpec { label: "grip_value", paths: &["/user/hand/left/input/squeeze/value", "/user/hand/left/input/grip/value"] },
];
const VALVE_INDEX_RIGHT_SPECS: &[BindingSpec] = &[
    BindingSpec { label: "right_stick_x", paths: &["/user/hand/right/input/thumbstick/x", "/user/hand/right/input/joystick/x"] },
    BindingSpec { label: "right_stick_y", paths: &["/user/hand/right/input/thumbstick/y", "/user/hand/right/input/joystick/y"] },
    BindingSpec { label: "trigger_value", paths: &["/user/hand/right/input/trigger/value"] },
    BindingSpec { label: "grip_value", paths: &["/user/hand/right/input/squeeze/value", "/user/hand/right/input/grip/value"] },
    BindingSpec { label: "a", paths: &["/user/hand/right/input/a/click"] },
    BindingSpec { label: "b", paths: &["/user/hand/right/input/b/click"] },
];

const MICROSOFT_MOTION_LEFT_SPECS: &[BindingSpec] = &[
    BindingSpec { label: "left_stick_x", paths: &["/user/hand/left/input/thumbstick/x", "/user/hand/left/input/joystick/x"] },
    BindingSpec { label: "left_stick_y", paths: &["/user/hand/left/input/thumbstick/y", "/user/hand/left/input/joystick/y"] },
    BindingSpec { label: "trigger_value", paths: &["/user/hand/left/input/trigger/value"] },
    BindingSpec { label: "grip_click", paths: &["/user/hand/left/input/squeeze/click", "/user/hand/left/input/grip/click"] },
];
const MICROSOFT_MOTION_RIGHT_SPECS: &[BindingSpec] = &[
    BindingSpec { label: "right_stick_x", paths: &["/user/hand/right/input/thumbstick/x", "/user/hand/right/input/joystick/x"] },
    BindingSpec { label: "right_stick_y", paths: &["/user/hand/right/input/thumbstick/y", "/user/hand/right/input/joystick/y"] },
    BindingSpec { label: "trigger_value", paths: &["/user/hand/right/input/trigger/value"] },
    BindingSpec { label: "grip_click", paths: &["/user/hand/right/input/squeeze/click", "/user/hand/right/input/grip/click"] },
];

const HTC_VIVE_LEFT_SPECS: &[BindingSpec] = &[
    BindingSpec { label: "trigger_click", paths: &["/user/hand/left/input/trigger/click"] },
    BindingSpec { label: "grip_click", paths: &["/user/hand/left/input/squeeze/click", "/user/hand/left/input/grip/click"] },
];
const HTC_VIVE_RIGHT_SPECS: &[BindingSpec] = &[
    BindingSpec { label: "trigger_click", paths: &["/user/hand/right/input/trigger/click"] },
    BindingSpec { label: "grip_click", paths: &["/user/hand/right/input/squeeze/click", "/user/hand/right/input/grip/click"] },
];

const HTC_VIVE_FOCUS3_LEFT_SPECS: &[BindingSpec] = &[
    BindingSpec { label: "left_stick_x", paths: &["/user/hand/left/input/thumbstick/x", "/user/hand/left/input/joystick/x"] },
    BindingSpec { label: "left_stick_y", paths: &["/user/hand/left/input/thumbstick/y", "/user/hand/left/input/joystick/y"] },
    BindingSpec { label: "trigger_value", paths: &["/user/hand/left/input/trigger/value"] },
    BindingSpec { label: "trigger_click", paths: &["/user/hand/left/input/trigger/click"] },
    BindingSpec { label: "grip_value", paths: &["/user/hand/left/input/squeeze/value", "/user/hand/left/input/grip/value"] },
    BindingSpec { label: "grip_click", paths: &["/user/hand/left/input/squeeze/click", "/user/hand/left/input/grip/click"] },
    BindingSpec { label: "x", paths: &["/user/hand/left/input/x/click"] },
    BindingSpec { label: "y", paths: &["/user/hand/left/input/y/click"] },
];
const HTC_VIVE_FOCUS3_RIGHT_SPECS: &[BindingSpec] = &[
    BindingSpec { label: "right_stick_x", paths: &["/user/hand/right/input/thumbstick/x", "/user/hand/right/input/joystick/x"] },
    BindingSpec { label: "right_stick_y", paths: &["/user/hand/right/input/thumbstick/y", "/user/hand/right/input/joystick/y"] },
    BindingSpec { label: "trigger_value", paths: &["/user/hand/right/input/trigger/value"] },
    BindingSpec { label: "trigger_click", paths: &["/user/hand/right/input/trigger/click"] },
    BindingSpec { label: "grip_value", paths: &["/user/hand/right/input/squeeze/value", "/user/hand/right/input/grip/value"] },
    BindingSpec { label: "grip_click", paths: &["/user/hand/right/input/squeeze/click", "/user/hand/right/input/grip/click"] },
    BindingSpec { label: "a", paths: &["/user/hand/right/input/a/click"] },
    BindingSpec { label: "b", paths: &["/user/hand/right/input/b/click"] },
];

const EXT_HAND_INTERACTION_LEFT_SPECS: &[BindingSpec] = &[];
const EXT_HAND_INTERACTION_RIGHT_SPECS: &[BindingSpec] = &[];

const PROFILE_BINDING_SPECS: &[ProfileBindingSpec] = &[
    ProfileBindingSpec {
        profile: PROFILE_SIMPLE_CONTROLLER,
        select_left: Some("/user/hand/left/input/select/click"),
        select_right: Some("/user/hand/right/input/select/click"),
        left_specs: SIMPLE_CONTROLLER_LEFT_SPECS,
        right_specs: SIMPLE_CONTROLLER_RIGHT_SPECS,
    },
    ProfileBindingSpec {
        profile: PROFILE_OCULUS_TOUCH,
        select_left: Some("/user/hand/left/input/trigger/click"),
        select_right: Some("/user/hand/right/input/trigger/click"),
        left_specs: OCULUS_TOUCH_LEFT_SPECS,
        right_specs: OCULUS_TOUCH_RIGHT_SPECS,
    },
    ProfileBindingSpec {
        profile: PROFILE_HTC_VIVE,
        select_left: Some("/user/hand/left/input/trigger/click"),
        select_right: Some("/user/hand/right/input/trigger/click"),
        left_specs: HTC_VIVE_LEFT_SPECS,
        right_specs: HTC_VIVE_RIGHT_SPECS,
    },
    ProfileBindingSpec {
        profile: PROFILE_HTC_VIVE_FOCUS3,
        select_left: Some("/user/hand/left/input/trigger/click"),
        select_right: Some("/user/hand/right/input/trigger/click"),
        left_specs: HTC_VIVE_FOCUS3_LEFT_SPECS,
        right_specs: HTC_VIVE_FOCUS3_RIGHT_SPECS,
    },
    ProfileBindingSpec {
        profile: PROFILE_VALVE_INDEX,
        select_left: Some("/user/hand/left/input/trigger/value"),
        select_right: Some("/user/hand/right/input/trigger/value"),
        left_specs: VALVE_INDEX_LEFT_SPECS,
        right_specs: VALVE_INDEX_RIGHT_SPECS,
    },
    ProfileBindingSpec {
        profile: PROFILE_MICROSOFT_MOTION,
        select_left: Some("/user/hand/left/input/trigger/value"),
        select_right: Some("/user/hand/right/input/trigger/value"),
        left_specs: MICROSOFT_MOTION_LEFT_SPECS,
        right_specs: MICROSOFT_MOTION_RIGHT_SPECS,
    },
    ProfileBindingSpec {
        profile: PROFILE_EXT_HAND_INTERACTION,
        select_left: Some("/user/hand/left/input/pinch_ext/value"),
        select_right: Some("/user/hand/right/input/pinch_ext/value"),
        left_specs: EXT_HAND_INTERACTION_LEFT_SPECS,
        right_specs: EXT_HAND_INTERACTION_RIGHT_SPECS,
    },
];

impl Default for OpenXRSystem {
    fn default() -> Self {
        Self {
            state: None,
            last_init_error: None,
            vulkan_graphics: None,
            preferred_swapchain_format: None,

            last_render_instant: None,
            last_render_dt_sec: None,

            input_xr_components: HashSet::new(),
            controller_components: HashSet::new(),
            controller_pose_source_last_logged: HashMap::new(),

            xr_input_state: XrInputState::default(),
            xr_gamepad_state: XrGamepadState::default(),
            xr_gamepad_state_last_logged: XrGamepadState::default(),
        }
    }
}

impl std::fmt::Debug for OpenXRSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenXRSystem")
            .field("initialized", &self.state.is_some())
            .field("last_init_error", &self.last_init_error)
            .field("last_render_dt_sec", &self.last_render_dt_sec)
            .finish()
    }
}

impl OpenXRSystem {
    pub fn xr_input_state(&self) -> &XrInputState {
        &self.xr_input_state
    }

    pub fn xr_gamepad_state(&self) -> &XrGamepadState {
        &self.xr_gamepad_state
    }

    pub fn last_init_error(&self) -> Option<&str> {
        self.last_init_error.as_deref()
    }

    fn debug_hand_rotation_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            std::env::var("CAT_DEBUG_XR_HAND_ROTATION")
                .ok()
                .map(|value| {
                    let value = value.trim().to_ascii_lowercase();
                    matches!(value.as_str(), "1" | "true" | "yes" | "on")
                })
                .unwrap_or(false)
        })
    }

    fn debug_hand_rotation_window_len() -> usize {
        static WINDOW_LEN: OnceLock<usize> = OnceLock::new();
        *WINDOW_LEN.get_or_init(|| {
            std::env::var("CAT_DEBUG_XR_HAND_ROTATION_WINDOW")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .map(|value| value.clamp(1, 600))
                .unwrap_or(60)
        })
    }

    fn quat_from_posef(pose: openxr::Posef) -> [f32; 4] {
        [
            pose.orientation.x,
            pose.orientation.y,
            pose.orientation.z,
            pose.orientation.w,
        ]
    }

    fn quat_normalize(q: [f32; 4]) -> [f32; 4] {
        let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
        if len > 0.0 {
            [q[0] / len, q[1] / len, q[2] / len, q[3] / len]
        } else {
            [0.0, 0.0, 0.0, 1.0]
        }
    }

    fn quat_nlerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
        let mut end = b;
        let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
        if dot < 0.0 {
            end = [-b[0], -b[1], -b[2], -b[3]];
        }
        let one_minus_t = 1.0 - t;
        Self::quat_normalize([
            a[0] * one_minus_t + end[0] * t,
            a[1] * one_minus_t + end[1] * t,
            a[2] * one_minus_t + end[2] * t,
            a[3] * one_minus_t + end[3] * t,
        ])
    }

    fn pose_from_quat_translation(
        rotation_xyzw: [f32; 4],
        translation_xyz: [f32; 3],
    ) -> openxr::Posef {
        openxr::Posef {
            orientation: openxr::Quaternionf {
                x: rotation_xyzw[0],
                y: rotation_xyzw[1],
                z: rotation_xyzw[2],
                w: rotation_xyzw[3],
            },
            position: openxr::Vector3f {
                x: translation_xyz[0],
                y: translation_xyz[1],
                z: translation_xyz[2],
            },
        }
    }

    fn derive_head_pose(views: &[openxr::View]) -> Option<openxr::Posef> {
        match views {
            [] => None,
            [view] => Some(view.pose),
            [left, right, ..] => {
                let left_q = Self::quat_from_posef(left.pose);
                let right_q = Self::quat_from_posef(right.pose);
                let center_q = Self::quat_nlerp(left_q, right_q, 0.5);
                let center_t = [
                    0.5 * (left.pose.position.x + right.pose.position.x),
                    0.5 * (left.pose.position.y + right.pose.position.y),
                    0.5 * (left.pose.position.z + right.pose.position.z),
                ];
                Some(Self::pose_from_quat_translation(center_q, center_t))
            }
        }
    }

    fn quat_angle_degrees(a: [f32; 4], b: [f32; 4]) -> f32 {
        let a = Self::quat_normalize(a);
        let b = Self::quat_normalize(b);
        let dot = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3])
            .abs()
            .clamp(0.0, 1.0);
        (2.0 * dot.acos()).to_degrees()
    }

    fn rolling_avg(window: &VecDeque<f32>) -> f32 {
        if window.is_empty() {
            0.0
        } else {
            window.iter().copied().sum::<f32>() / window.len() as f32
        }
    }

    fn rolling_max(window: &VecDeque<f32>) -> f32 {
        window.iter().copied().fold(0.0, f32::max)
    }

    fn update_hand_rotation_debug(
        debug_state: &mut HandRotationDebugState,
        hand: ControllerHand,
        _joint: Option<openxr::HandJointEXT>,
        pose: openxr::Posef,
    ) {
        if !Self::debug_hand_rotation_enabled() {
            return;
        }

        let quat = Self::quat_from_posef(pose);
        let window_len = Self::debug_hand_rotation_window_len();
        let debug = match hand {
            ControllerHand::Left => &mut debug_state.left,
            ControllerHand::Right => &mut debug_state.right,
        };

        let step_deg = debug
            .previous_quat_xyzw
            .map(|previous| Self::quat_angle_degrees(previous, quat))
            .unwrap_or(0.0);
        debug.previous_quat_xyzw = Some(quat);

        if debug.step_deg.len() >= window_len {
            let _ = debug.step_deg.pop_front();
        }
        debug.step_deg.push_back(step_deg);
        debug.sample_count += 1;

        let _ = window_len;
    }

    fn preferred_pose(
        sess: &OpenXRSessionState,
        hand: ControllerHand,
        pose: ControllerPoseKind,
    ) -> (Option<openxr::Posef>, &'static str) {
        let hand_root = match hand {
            ControllerHand::Left => sess.hand_root_pose_cache.left_root,
            ControllerHand::Right => sess.hand_root_pose_cache.right_root,
        };
        if let Some(pose) = hand_root {
            return (Some(pose), "hand_root");
        }

        let controller_pose = match (hand, pose) {
            (ControllerHand::Left, ControllerPoseKind::Aim) => sess.controller_pose_cache.left_aim,
            (ControllerHand::Right, ControllerPoseKind::Aim) => {
                sess.controller_pose_cache.right_aim
            }
            (ControllerHand::Left, ControllerPoseKind::Grip) => {
                sess.controller_pose_cache.left_grip
            }
            (ControllerHand::Right, ControllerPoseKind::Grip) => {
                sess.controller_pose_cache.right_grip
            }
        };

        if controller_pose.is_some() {
            (controller_pose, "controller_action")
        } else {
            (None, "none")
        }
    }

    fn transform_child_of(world: &World, component: ComponentId) -> Option<ComponentId> {
        world.children_of(component).iter().copied().find(|&cid| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(cid)
                .is_some()
        })
    }

    fn transform_parent_world(world: &World, transform_cid: ComponentId) -> [[f32; 4]; 4] {
        world
            .parent_of(transform_cid)
            .and_then(|parent| TransformSystem::world_model(world, parent))
            .unwrap_or([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ])
    }

    fn input_xr_ancestor(world: &World, cid: ComponentId) -> Option<ComponentId> {
        let mut cur = cid;
        loop {
            if world
                .get_component_by_id_as::<InputVRComponent>(cur)
                .is_some()
            {
                return Some(cur);
            }
            let Some(parent) = world.parent_of(cur) else {
                return None;
            };
            cur = parent;
        }
    }

    fn xr_rig_origin_world(world: &World, visuals: &VisualWorld) -> [[f32; 4]; 4] {
        let Some(camera_cid) = visuals
            .active_xr_camera()
            .or_else(|| Self::first_enabled_camera_xr(world))
        else {
            return [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ];
        };

        if let Some(input_xr_cid) = Self::input_xr_ancestor(world, camera_cid) {
            if let Some(driven_transform) = Self::transform_child_of(world, input_xr_cid) {
                return Self::transform_parent_world(world, driven_transform);
            }
        }

        TransformSystem::world_model(world, camera_cid).unwrap_or([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    /// Hint: prefer this Vulkan `VkFormat` (as raw `u32`) when creating the OpenXR swapchain.
    ///
    /// This is used to match the window renderer's color attachment format, so we can copy
    /// rendered images into the OpenXR swapchain without format mismatch.
    pub fn set_preferred_swapchain_format(&mut self, format: u32) {
        self.preferred_swapchain_format = Some(format);
    }

    /// Returns the Vulkan instance/device extensions required by the active OpenXR runtime.
    ///
    /// This uses the `XR_KHR_vulkan_enable` query APIs (space-delimited strings). Even when the
    /// runtime supports `XR_KHR_vulkan_enable2`, SteamVR commonly still reports the required
    /// extension lists via these functions.
    pub fn required_vulkan_extensions(&self) -> Option<(Vec<String>, Vec<String>)> {
        let state = self.state.as_ref()?;

        let instance_exts = state
            .instance
            .vulkan_legacy_instance_extensions(state.system)
            .ok()?;
        let device_exts = state
            .instance
            .vulkan_legacy_device_extensions(state.system)
            .ok()?;

        let split = |s: String| -> Vec<String> {
            s.split_whitespace()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect()
        };

        Some((split(instance_exts), split(device_exts)))
    }

    pub fn set_vulkan_graphics(&mut self, gfx: XrVulkanGraphics) {
        self.vulkan_graphics = Some(gfx);

        // If OpenXR is already initialized, opportunistically create the session now.
        let Some(state) = self.state.as_mut() else {
            return;
        };

        if state.session.is_none() {
            if let Err(err) = Self::try_init_session(state, gfx, self.preferred_swapchain_format) {
                eprintln!("[OpenXR] Session init failed: {err}");
                self.last_init_error = Some(err);
            }
        }
    }

    pub fn initialize_runtime(&mut self) -> Result<(), String> {
        if self.state.is_some() {
            return Ok(());
        }

        match Self::try_init_openxr() {
            Ok(state) => {
                println!("[OpenXR] Initialized.");
                self.state = Some(state);
                self.last_init_error = None;

                if let (Some(state), Some(gfx)) = (self.state.as_mut(), self.vulkan_graphics) {
                    if state.session.is_none() {
                        if let Err(err) =
                            Self::try_init_session(state, gfx, self.preferred_swapchain_format)
                        {
                            eprintln!("[OpenXR] Session init failed: {err}");
                            self.last_init_error = Some(err.clone());
                            return Err(err);
                        }
                    }
                }

                Ok(())
            }
            Err(err) => {
                eprintln!("[OpenXR] Init failed: {err}");
                self.last_init_error = Some(err.clone());
                Err(err)
            }
        }
    }

    pub fn register_vr(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let Some(cfg) = world.get_component_by_id_as::<VrComponent>(component) else {
            return;
        };

        if !cfg.enabled {
            return;
        }

        if self.state.is_some() {
            return;
        }

        let _ = self.initialize_runtime();
    }

    pub fn register_controller_xr(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.controller_components.insert(component);
        self.controller_pose_source_last_logged.remove(&component);
    }

    pub fn register_input_xr(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.input_xr_components.insert(component);
    }

    pub fn remove_controller_xr(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.controller_components.remove(&component);
        self.controller_pose_source_last_logged.remove(&component);
    }

    pub fn remove_input_xr(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.input_xr_components.remove(&component);
    }

    fn pump_events(&mut self) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        // Drain events; for now we just print key session state transitions.
        loop {
            let evt = match state.instance.poll_event(&mut state.events) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[OpenXR] poll_event error: {e:?}");
                    return;
                }
            };

            let Some(evt) = evt else {
                break;
            };

            match evt {
                openxr::Event::InstanceLossPending(_) => {
                    eprintln!("[OpenXR] Event: InstanceLossPending");
                }
                openxr::Event::SessionStateChanged(e) => {
                    println!("[OpenXR] Event: SessionStateChanged -> {:?}", e.state());

                    if let Some(sess) = state.session.as_mut() {
                        sess.current_state = e.state();
                        match e.state() {
                            openxr::SessionState::READY => {
                                if !sess.running {
                                    if let Err(err) = sess.session.begin(state.view_type) {
                                        eprintln!("[OpenXR] session.begin failed: {err:?}");
                                    } else {
                                        sess.running = true;
                                    }
                                }
                            }
                            openxr::SessionState::STOPPING => {
                                if sess.running {
                                    if let Err(err) = sess.session.end() {
                                        eprintln!("[OpenXR] session.end failed: {err:?}");
                                    }
                                    sess.running = false;
                                }
                            }
                            openxr::SessionState::EXITING | openxr::SessionState::LOSS_PENDING => {
                                sess.running = false;
                            }
                            _ => {}
                        }
                    }
                }
                openxr::Event::EventsLost(e) => {
                    eprintln!("[OpenXR] Event: EventsLost ({})", e.lost_event_count());
                }
                _ => {
                    // Too noisy to print everything by default.
                }
            }
        }
    }

    /// Like `tick`, but also queues transform updates for registered ControllerXRComponents.
    ///
    /// Controller poses are sourced from the last cache update performed during `render_xr`.
    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        _input: &InputState,
        emit: &mut dyn SignalEmitter,
        _dt_sec: f32,
    ) {
        self.pump_events();

        let Some(state) = self.state.as_ref() else {
            return;
        };
        let Some(sess) = state.session.as_ref() else {
            return;
        };
        if !sess.running {
            return;
        }

        // Compose headset/controller poses with the authored XR rig origin.
        let rig_world = Self::xr_rig_origin_world(world, visuals);

        let input_xr_ids: Vec<ComponentId> = self.input_xr_components.iter().copied().collect();
        for input_xr_cid in input_xr_ids {
            let Some(cfg) = world.get_component_by_id_as::<InputVRComponent>(input_xr_cid) else {
                self.input_xr_components.remove(&input_xr_cid);
                continue;
            };

            if !cfg.enabled {
                continue;
            }

            let Some(head_pose) = sess.head_pose_cache else {
                continue;
            };

            let Some(tcid) = Self::transform_child_of(world, input_xr_cid) else {
                continue;
            };

            let world_from_head = Self::mul_mat4(
                &Self::transform_parent_world(world, tcid),
                &Self::mat4_from_pose(head_pose),
            );
            let desired_world_pos = [
                world_from_head[3][0],
                world_from_head[3][1],
                world_from_head[3][2],
            ];
            let desired_world_rot = math::mat_to_quat(world_from_head);

            let local_translation =
                Self::world_to_local_translation(world, tcid, desired_world_pos);
            let parent_world_rot =
                Self::parent_world_rotation_quat(world, tcid).unwrap_or([0.0, 0.0, 0.0, 1.0]);
            let local_rotation =
                math::quat_mul(math::quat_conjugate(parent_world_rot), desired_world_rot);

            let Some(t) = world
                .get_component_by_id_as_mut::<crate::engine::ecs::component::TransformComponent>(
                    tcid,
                )
            else {
                continue;
            };

            t.transform.translation = local_translation;
            t.transform.rotation = local_rotation;
            t.transform.recompute_model();

            let transform = t.transform;
            emit.push_intent_now(
                tcid,
                IntentValue::UpdateTransform {
                    component_ids: vec![tcid],
                    translation: transform.translation,
                    rotation_quat_xyzw: transform.rotation,
                    scale: transform.scale,
                },
            );
        }

        let controller_ids: Vec<ComponentId> = self.controller_components.iter().copied().collect();
        for controller_cid in controller_ids {
            let Some(cfg) = world.get_component_by_id_as::<VRHandComponent>(controller_cid)
            else {
                self.controller_components.remove(&controller_cid);
                continue;
            };

            if !cfg.enabled {
                continue;
            }

            let (pose, pose_source) = Self::preferred_pose(sess, cfg.hand, cfg.pose);
            let last_pose_source = self
                .controller_pose_source_last_logged
                .get(&controller_cid)
                .copied();
            if openxr_debug_enabled() && last_pose_source != Some(pose_source) {
                eprintln!(
                    "[OpenXR][ctlxr] component={controller_cid:?} hand={:?} pose={:?} source={pose_source}",
                    cfg.hand, cfg.pose
                );
                self.controller_pose_source_last_logged
                    .insert(controller_cid, pose_source);
            }

            let Some(pose) = pose else {
                continue;
            };

            // Semantics: ControllerXRComponent drives a TransformComponent child, if present.
            let Some(tcid) = Self::transform_child_of(world, controller_cid) else {
                continue;
            };

            // Pose is in OpenXR reference space; convert to engine world by applying rig transform.
            let world_from_controller = Self::mul_mat4(&rig_world, &Self::mat4_from_pose(pose));
            let desired_world_pos = [
                world_from_controller[3][0],
                world_from_controller[3][1],
                world_from_controller[3][2],
            ];
            let desired_world_rot = math::mat_to_quat(world_from_controller);

            let local_translation =
                Self::world_to_local_translation(world, tcid, desired_world_pos);
            let parent_world_rot =
                Self::parent_world_rotation_quat(world, tcid).unwrap_or([0.0, 0.0, 0.0, 1.0]);
            let local_rotation =
                math::quat_mul(math::quat_conjugate(parent_world_rot), desired_world_rot);

            let Some(t) = world
                .get_component_by_id_as_mut::<crate::engine::ecs::component::TransformComponent>(
                    tcid,
                )
            else {
                continue;
            };

            // Convert world-space target into local-space values relative to the nearest parent
            // transform above `tcid` (if any), matching how transform chains are composed.
            t.transform.translation = local_translation;
            t.transform.rotation = local_rotation;
            t.transform.recompute_model();

            let transform = t.transform;
            emit.push_intent_now(
                tcid,
                IntentValue::UpdateTransform {
                    component_ids: vec![tcid],
                    translation: transform.translation,
                    rotation_quat_xyzw: transform.rotation,
                    scale: transform.scale,
                },
            );
        }
    }

    fn world_to_local_translation(
        world: &World,
        transform_cid: ComponentId,
        desired_world: [f32; 3],
    ) -> [f32; 3] {
        let mut cur = transform_cid;
        while let Some(parent) = world.parent_of(cur) {
            if let Some(t) = world
                .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(parent)
            {
                if let Some(inv) = math::mat4_inverse(t.transform.matrix_world) {
                    let p_local = math::mat4_mul_vec4(
                        inv,
                        [desired_world[0], desired_world[1], desired_world[2], 1.0],
                    );
                    return [p_local[0], p_local[1], p_local[2]];
                }
                break;
            }
            cur = parent;
        }

        desired_world
    }

    fn parent_world_rotation_quat(world: &World, transform_cid: ComponentId) -> Option<[f32; 4]> {
        let mut cur = transform_cid;
        while let Some(parent) = world.parent_of(cur) {
            if let Some(t) = world
                .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(parent)
            {
                return Some(math::mat_to_quat(t.transform.matrix_world));
            }
            cur = parent;
        }
        None
    }

    fn try_init_hand_tracking(
        session: &openxr::Session<openxr::Vulkan>,
    ) -> Result<HandTrackingState, String> {
        let left = session
            .create_hand_tracker(openxr::HandEXT::LEFT)
            .map_err(|e| format!("create_hand_tracker(left): {e:?}"))?;
        let right = session
            .create_hand_tracker(openxr::HandEXT::RIGHT)
            .map_err(|e| format!("create_hand_tracker(right): {e:?}"))?;

        Ok(HandTrackingState { left, right })
    }

    fn valid_pose_from_hand_joint(joint: &openxr::HandJointLocationEXT) -> Option<openxr::Posef> {
        let flags = joint.location_flags;
        if flags.contains(openxr::SpaceLocationFlags::POSITION_VALID)
            && flags.contains(openxr::SpaceLocationFlags::ORIENTATION_VALID)
        {
            Some(joint.pose)
        } else {
            None
        }
    }

    fn select_hand_root_pose(
        joints: &openxr::HandJointLocations,
    ) -> Option<(openxr::Posef, openxr::HandJointEXT)> {
        let wrist = &joints[openxr::HandJointEXT::WRIST];
        if let Some(pose) = Self::valid_pose_from_hand_joint(wrist) {
            return Some((pose, openxr::HandJointEXT::WRIST));
        }

        let palm = &joints[openxr::HandJointEXT::PALM];
        Self::valid_pose_from_hand_joint(palm).map(|pose| (pose, openxr::HandJointEXT::PALM))
    }

    fn wrist_palm_delta_deg(joints: &openxr::HandJointLocations) -> Option<u16> {
        let wrist = Self::valid_pose_from_hand_joint(&joints[openxr::HandJointEXT::WRIST])?;
        let palm = Self::valid_pose_from_hand_joint(&joints[openxr::HandJointEXT::PALM])?;
        let deg = Self::quat_angle_degrees(Self::quat_from_posef(wrist), Self::quat_from_posef(palm));
        Some(deg.round().clamp(0.0, u16::MAX as f32) as u16)
    }

    fn quantize_angle_deg(value: u16, bucket_size: u16) -> u16 {
        if bucket_size <= 1 {
            value
        } else {
            (value / bucket_size) * bucket_size
        }
    }

    fn root_pose_for_joint(
        joints: &openxr::HandJointLocations,
        joint: Option<openxr::HandJointEXT>,
    ) -> Option<openxr::Posef> {
        let joint = joint?;
        Self::valid_pose_from_hand_joint(&joints[joint])
    }

    fn hand_debug_state(
        joints: Option<&openxr::HandJointLocations>,
        root_joint: Option<openxr::HandJointEXT>,
        previous_root_quat_xyzw: Option<[f32; 4]>,
    ) -> HandDebugState {
        let wrist_palm_delta_deg = joints
            .and_then(Self::wrist_palm_delta_deg)
            .map(|deg| Self::quantize_angle_deg(deg, 5));
        let root_pose = joints.and_then(|j| Self::root_pose_for_joint(j, root_joint));
        let wrist_step_deg = match (previous_root_quat_xyzw, root_pose) {
            (Some(previous), Some(pose)) => Some(
                Self::quat_angle_degrees(previous, Self::quat_from_posef(pose))
                    .round()
                    .clamp(0.0, u16::MAX as f32) as u16,
            ),
            _ => None,
        }
        .map(|deg| Self::quantize_angle_deg(deg, 2));
        HandDebugState {
            root_joint,
            wrist_palm_delta_deg,
            wrist_step_deg,
            wrist_step_spike: wrist_step_deg.is_some_and(|deg| deg >= 8),
        }
    }

    fn log_hand_debug_snapshot(
        sess: &mut OpenXRSessionState,
        left_joints: Option<&openxr::HandJointLocations>,
        right_joints: Option<&openxr::HandJointLocations>,
        left_root_joint: Option<openxr::HandJointEXT>,
        right_root_joint: Option<openxr::HandJointEXT>,
    ) {
        let snapshot = HandDebugSnapshot {
            left: Self::hand_debug_state(
                left_joints,
                left_root_joint,
                sess.hand_rotation_debug.left.previous_quat_xyzw,
            ),
            right: Self::hand_debug_state(
                right_joints,
                right_root_joint,
                sess.hand_rotation_debug.right.previous_quat_xyzw,
            ),
        };
        if sess.last_hand_debug_snapshot == Some(snapshot) {
            return;
        }
        sess.last_hand_debug_snapshot = Some(snapshot);
        if openxr_debug_enabled() {
            eprintln!(
                "[OpenXR][hand] left root={:?} wrist_palm_delta_deg={:?} wrist_step_deg={:?} spike={} | right root={:?} wrist_palm_delta_deg={:?} wrist_step_deg={:?} spike={}",
                snapshot.left.root_joint,
                snapshot.left.wrist_palm_delta_deg,
                snapshot.left.wrist_step_deg,
                snapshot.left.wrist_step_spike,
                snapshot.right.root_joint,
                snapshot.right.wrist_palm_delta_deg,
                snapshot.right.wrist_step_deg,
                snapshot.right.wrist_step_spike,
            );
        }
    }

    fn try_init_openxr() -> Result<OpenXRState, String> {
        // Prefer dynamically loading the OpenXR loader. This keeps us from requiring
        // special linker setup and matches typical Linux setups.
        let entry = unsafe { openxr::Entry::load().map_err(|e| format!("Entry::load: {e:?}"))? };

        let available_extensions = entry
            .enumerate_extensions()
            .map_err(|e| format!("enumerate_extensions: {e:?}"))?;

        let app_info = openxr::ApplicationInfo {
            application_name: "cat-engine",
            application_version: 1,
            engine_name: "cat-engine",
            engine_version: 1,
            api_version: openxr::Version::new(1, 0, 0),
        };

        let mut extensions = openxr::ExtensionSet::default();
        // Use the legacy Vulkan binding path for now (reuse an already-created VkInstance/VkDevice).
        // SteamVR can be stricter with XR_KHR_vulkan_enable2 unless you create Vulkan objects
        // via xrCreateVulkanInstanceKHR/xrCreateVulkanDeviceKHR.
        extensions.khr_vulkan_enable2 = false;
        // Needed for Vulkan session creation.
        extensions.khr_vulkan_enable = true;
        if available_extensions.ext_hand_tracking {
            extensions.ext_hand_tracking = true;
        }
        if available_extensions.ext_hand_interaction {
            extensions.ext_hand_interaction = true;
        }
        if available_extensions.htc_vive_focus3_controller_interaction {
            extensions.htc_vive_focus3_controller_interaction = true;
        }
        if available_extensions.htc_hand_interaction {
            extensions.htc_hand_interaction = true;
        }
        let layers: [&str; 0] = [];

        let instance = entry
            .create_instance(&app_info, &extensions, &layers)
            .map_err(|e| format!("create_instance: {e:?}"))?;

        println!(
            "[OpenXR] Enabled extensions: khr_vulkan_enable={}, ext_hand_tracking={}, ext_hand_interaction={}, htc_vive_focus3_controller_interaction={}, htc_hand_interaction={}",
            extensions.khr_vulkan_enable,
            extensions.ext_hand_tracking,
            extensions.ext_hand_interaction,
            extensions.htc_vive_focus3_controller_interaction,
            extensions.htc_hand_interaction,
        );

        // Best-effort runtime identification (helps debugging which OpenXR runtime is active).
        if let Ok(props) = instance.properties() {
            println!(
                "[OpenXR] Runtime: {} ({:?})",
                props.runtime_name, props.runtime_version
            );
        }

        let system = match instance.system(openxr::FormFactor::HEAD_MOUNTED_DISPLAY) {
            Ok(system) => system,
            Err(openxr::sys::Result::ERROR_FORM_FACTOR_UNAVAILABLE) => {
                return Err(
                    "system(HMD): ERROR_FORM_FACTOR_UNAVAILABLE (no HMD detected / runtime not ready).\n\
                    Start an OpenXR runtime (e.g. SteamVR/Monado/ALVR) and ensure a headset is connected and the runtime is running before launching cat-engine.\n\
                    On Linux you can also check XR_RUNTIME_JSON points to an installed runtime manifest."
                        .to_string(),
                );
            }
            Err(e) => return Err(format!("system(HMD): {e:?}")),
        };

        // Pick a supported blend mode and stash common config.
        let view_type = openxr::ViewConfigurationType::PRIMARY_STEREO;
        let blend_mode = instance
            .enumerate_environment_blend_modes(system, view_type)
            .ok()
            .and_then(|m| m.first().copied())
            .unwrap_or(openxr::EnvironmentBlendMode::OPAQUE);

        Ok(OpenXRState {
            entry,
            instance,
            system,
            events: openxr::EventDataBuffer::new(),
            session: None,
            view_type,
            blend_mode,
        })
    }

    fn try_init_session(
        state: &mut OpenXRState,
        gfx: XrVulkanGraphics,
        preferred_swapchain_format: Option<u32>,
    ) -> Result<(), String> {
        // Log Vulkan version requirements (useful debugging).
        if let Ok(reqs) = state
            .instance
            .graphics_requirements::<openxr::Vulkan>(state.system)
        {
            println!(
                "[OpenXR] Vulkan requirements: min {:?}, max {:?}",
                reqs.min_api_version_supported, reqs.max_api_version_supported
            );
        }

        // SteamVR may validate that the VkPhysicalDevice matches the one it expects for the HMD.
        // If these differ (multi-GPU setups), create_session can fail.
        if let Ok(required_pd) = unsafe {
            state
                .instance
                .vulkan_graphics_device(state.system, gfx.vk_instance)
        } {
            if required_pd != gfx.vk_physical_device {
                return Err(format!(
                    "Vulkan physical device mismatch for OpenXR system.\n\
Engine/Vulkano physical device: {:?}\n\
OpenXR-required physical device: {:?}\n\
Fix: pick the OpenXR-required VkPhysicalDevice when creating the Vulkan device/queues.",
                    gfx.vk_physical_device, required_pd
                ));
            }
        }

        let info = openxr::vulkan::SessionCreateInfo {
            instance: gfx.vk_instance,
            physical_device: gfx.vk_physical_device,
            device: gfx.vk_device,
            queue_family_index: gfx.queue_family_index,
            queue_index: gfx.queue_index,
        };

        let (session, frame_waiter, frame_stream) = unsafe {
            state
                .instance
                .create_session::<openxr::Vulkan>(state.system, &info)
                .map_err(|e| {
                    let required_instance = state
                        .instance
                        .vulkan_legacy_instance_extensions(state.system)
                        .ok();
                    let required_device = state
                        .instance
                        .vulkan_legacy_device_extensions(state.system)
                        .ok();

                    let extra = match (required_instance, required_device) {
                        (Some(i), Some(d)) => format!(
                            "\n[OpenXR] Runtime-required Vulkan instance extensions: {i}\n[OpenXR] Runtime-required Vulkan device extensions: {d}"
                        ),
                        _ => String::new(),
                    };

                    format!(
                        "create_session(Vulkan): {e:?}.\n\
If this fails with Vulkan extension errors, the Vulkan instance/device created by Vulkano may be missing runtime-required extensions (from XR_KHR_vulkan_enable2)."
                    )
                    + extra.as_str()
                })?
        };

        let reference_space = session
            .create_reference_space(openxr::ReferenceSpaceType::STAGE, openxr::Posef::IDENTITY)
            .map_err(|e| format!("create_reference_space(STAGE): {e:?}"))?;

        let controller_input = match Self::try_init_controller_input(&state.instance, &session) {
            Ok(v) => {
                println!("[OpenXR] Controller input initialized.");
                Some(v)
            }
            Err(err) => {
                eprintln!("[OpenXR] Controller input init failed: {err}");
                None
            }
        };

        let hand_tracking = match state.instance.supports_hand_tracking(state.system) {
            Ok(v) => {
                println!("[OpenXR] Runtime supports hand tracking: {v}");
                if v {
                    match Self::try_init_hand_tracking(&session) {
                        Ok(trackers) => {
                            println!("[OpenXR] Hand tracking initialized.");
                            Some(trackers)
                        }
                        Err(err) => {
                            eprintln!("[OpenXR] Hand tracking init failed: {err}");
                            None
                        }
                    }
                } else {
                    None
                }
            }
            Err(e) => {
                eprintln!("[OpenXR] supports_hand_tracking query failed: {e:?}");
                None
            }
        };

        let xr_swapchain = XRSwapchain::new(
            &state.instance,
            &session,
            state.system,
            state.view_type,
            preferred_swapchain_format,
        )?;

        let swapchain_image_initialized = vec![false; xr_swapchain.images().len()];

        // Minimal Vulkan command recording for XR bring-up: wrap the existing VkDevice via ash.
        let entry = unsafe { ash::Entry::load().map_err(|e| format!("ash::Entry::load: {e:?}"))? };

        let vk_instance = ash::vk::Instance::from_raw(gfx.vk_instance as usize as u64);
        let vk_device_handle = ash::vk::Device::from_raw(gfx.vk_device as usize as u64);

        let vk_instance = unsafe { ash::Instance::load(entry.static_fn(), vk_instance) };
        let vk_device = unsafe { ash::Device::load(vk_instance.fp_v1_0(), vk_device_handle) };
        let vk_queue =
            unsafe { vk_device.get_device_queue(gfx.queue_family_index, gfx.queue_index) };

        let vk_command_pool = unsafe {
            vk_device
                .create_command_pool(
                    &ash::vk::CommandPoolCreateInfo::default()
                        .queue_family_index(gfx.queue_family_index)
                        .flags(ash::vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                    None,
                )
                .map_err(|e| format!("create_command_pool: {e:?}"))?
        };

        let vk_command_buffer = unsafe {
            vk_device
                .allocate_command_buffers(
                    &ash::vk::CommandBufferAllocateInfo::default()
                        .command_pool(vk_command_pool)
                        .level(ash::vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1),
                )
                .map_err(|e| format!("allocate_command_buffers: {e:?}"))?
                .into_iter()
                .next()
                .ok_or("allocate_command_buffers returned 0 command buffers")?
        };

        state.session = Some(OpenXRSessionState {
            session,
            frame_waiter,
            frame_stream,
            reference_space,
            running: false,
            current_state: openxr::SessionState::IDLE,

            xr_swapchain,

            swapchain_image_initialized,

            did_log_format_mismatch: false,

            vk_device,
            vk_queue,
            vk_command_pool,
            vk_command_buffer,

            hand_tracking,
            hand_root_pose_cache: HandRootPoseCache::default(),
            hand_rotation_debug: HandRotationDebugState::default(),
            last_hand_debug_snapshot: None,
            head_pose_cache: None,
            controller_input,
            controller_pose_cache: ControllerPoseCache::default(),
        });

        println!("[OpenXR] Session created (Vulkan)");
        Ok(())
    }

    fn try_init_controller_input(
        instance: &openxr::Instance,
        session: &openxr::Session<openxr::Vulkan>,
    ) -> Result<ControllerInput, String> {
        let left = instance
            .string_to_path("/user/hand/left")
            .map_err(|e| format!("string_to_path(/user/hand/left): {e:?}"))?;
        let right = instance
            .string_to_path("/user/hand/right")
            .map_err(|e| format!("string_to_path(/user/hand/right): {e:?}"))?;

        let action_set = instance
            .create_action_set("cat_engine", "Cat Engine", 0)
            .map_err(|e| format!("create_action_set: {e:?}"))?;
        let hand_subaction_paths = [left, right];

        let aim_pose = action_set
            .create_action::<openxr::Posef>("aim_pose", "Aim Pose", &hand_subaction_paths)
            .map_err(|e| format!("create_action(aim_pose): {e:?}"))?;
        let grip_pose = action_set
            .create_action::<openxr::Posef>("grip_pose", "Grip Pose", &hand_subaction_paths)
            .map_err(|e| format!("create_action(grip_pose): {e:?}"))?;
        let select = action_set
            .create_action::<bool>("select", "Select", &hand_subaction_paths)
            .map_err(|e| format!("create_action(select): {e:?}"))?;
        let left_stick_x = action_set
            .create_action::<f32>("left_stick_x", "Left Stick X", &hand_subaction_paths)
            .map_err(|e| format!("create_action(left_stick_x): {e:?}"))?;
        let left_stick_y = action_set
            .create_action::<f32>("left_stick_y", "Left Stick Y", &hand_subaction_paths)
            .map_err(|e| format!("create_action(left_stick_y): {e:?}"))?;
        let right_stick_x = action_set
            .create_action::<f32>("right_stick_x", "Right Stick X", &hand_subaction_paths)
            .map_err(|e| format!("create_action(right_stick_x): {e:?}"))?;
        let right_stick_y = action_set
            .create_action::<f32>("right_stick_y", "Right Stick Y", &hand_subaction_paths)
            .map_err(|e| format!("create_action(right_stick_y): {e:?}"))?;
        let trigger_value = action_set
            .create_action::<f32>("trigger_value", "Trigger Value", &hand_subaction_paths)
            .map_err(|e| format!("create_action(trigger_value): {e:?}"))?;
        let trigger_click = action_set
            .create_action::<bool>("trigger_click", "Trigger Click", &hand_subaction_paths)
            .map_err(|e| format!("create_action(trigger_click): {e:?}"))?;
        let grip_value = action_set
            .create_action::<f32>("grip_value", "Grip Value", &hand_subaction_paths)
            .map_err(|e| format!("create_action(grip_value): {e:?}"))?;
        let grip_click = action_set
            .create_action::<bool>("grip_click", "Grip Click", &hand_subaction_paths)
            .map_err(|e| format!("create_action(grip_click): {e:?}"))?;
        let button_a = action_set
            .create_action::<bool>("button_a", "Button A", &hand_subaction_paths)
            .map_err(|e| format!("create_action(button_a): {e:?}"))?;
        let button_b = action_set
            .create_action::<bool>("button_b", "Button B", &hand_subaction_paths)
            .map_err(|e| format!("create_action(button_b): {e:?}"))?;
        let button_x = action_set
            .create_action::<bool>("button_x", "Button X", &hand_subaction_paths)
            .map_err(|e| format!("create_action(button_x): {e:?}"))?;
        let button_y = action_set
            .create_action::<bool>("button_y", "Button Y", &hand_subaction_paths)
            .map_err(|e| format!("create_action(button_y): {e:?}"))?;

        let left_aim_space = aim_pose
            .create_space(session.clone(), left, openxr::Posef::IDENTITY)
            .map_err(|e| format!("aim_pose.create_space(left): {e:?}"))?;
        let right_aim_space = aim_pose
            .create_space(session.clone(), right, openxr::Posef::IDENTITY)
            .map_err(|e| format!("aim_pose.create_space(right): {e:?}"))?;
        let left_grip_space = grip_pose
            .create_space(session.clone(), left, openxr::Posef::IDENTITY)
            .map_err(|e| format!("grip_pose.create_space(left): {e:?}"))?;
        let right_grip_space = grip_pose
            .create_space(session.clone(), right, openxr::Posef::IDENTITY)
            .map_err(|e| format!("grip_pose.create_space(right): {e:?}"))?;

        // Suggest bindings before attaching the action set. Once attached, action sets
        // become immutable, and hiding suggestion failures makes profile bring-up
        // impossible to debug on SteamVR.
        let left_aim_path = instance
            .string_to_path("/user/hand/left/input/aim/pose")
            .map_err(|e| format!("string_to_path(left aim): {e:?}"))?;
        let right_aim_path = instance
            .string_to_path("/user/hand/right/input/aim/pose")
            .map_err(|e| format!("string_to_path(right aim): {e:?}"))?;
        let left_grip_path = instance
            .string_to_path("/user/hand/left/input/grip/pose")
            .map_err(|e| format!("string_to_path(left grip): {e:?}"))?;
        let right_grip_path = instance
            .string_to_path("/user/hand/right/input/grip/pose")
            .map_err(|e| format!("string_to_path(right grip): {e:?}"))?;

        // Pose bindings are common across all profiles.
        let pose_bindings = [
            openxr::Binding::new(&aim_pose, left_aim_path),
            openxr::Binding::new(&aim_pose, right_aim_path),
            openxr::Binding::new(&grip_pose, left_grip_path),
            openxr::Binding::new(&grip_pose, right_grip_path),
        ];

        for spec in PROFILE_BINDING_SPECS {
            log_profile_binding_dump(spec);
            let Ok(profile) = instance.string_to_path(spec.profile) else {
                continue;
            };
            for (binding_label, binding_path, binding) in [
                ("aim_pose_left", "/user/hand/left/input/aim/pose", pose_bindings[0]),
                ("aim_pose_right", "/user/hand/right/input/aim/pose", pose_bindings[1]),
                ("grip_pose_left", "/user/hand/left/input/grip/pose", pose_bindings[2]),
                ("grip_pose_right", "/user/hand/right/input/grip/pose", pose_bindings[3]),
            ] {
                suggest_binding_best_effort(
                    instance,
                    profile,
                    spec.profile,
                    binding_label,
                    binding_path,
                    binding,
                )?;
            }
            if let (Some(l), Some(r)) = (spec.select_left, spec.select_right) {
                if let (Ok(lp), Ok(rp)) = (instance.string_to_path(l), instance.string_to_path(r))
                {
                    suggest_binding_best_effort(
                        instance,
                        profile,
                        spec.profile,
                        "select_left",
                        l,
                        openxr::Binding::new(&select, lp),
                    )?;
                    suggest_binding_best_effort(
                        instance,
                        profile,
                        spec.profile,
                        "select_right",
                        r,
                        openxr::Binding::new(&select, rp),
                    )?;
                }
            }
            let mut suggested = Vec::new();
            for hand_spec in spec.left_specs {
                append_action_bindings(
                    instance,
                    &mut suggested,
                    *hand_spec,
                    &left_stick_x,
                    &left_stick_y,
                    &right_stick_x,
                    &right_stick_y,
                    &trigger_value,
                    &trigger_click,
                    &grip_value,
                    &grip_click,
                    &button_a,
                    &button_b,
                    &button_x,
                    &button_y,
                );
            }
            for hand_spec in spec.right_specs {
                append_action_bindings(
                    instance,
                    &mut suggested,
                    *hand_spec,
                    &left_stick_x,
                    &left_stick_y,
                    &right_stick_x,
                    &right_stick_y,
                    &trigger_value,
                    &trigger_click,
                    &grip_value,
                    &grip_click,
                    &button_a,
                    &button_b,
                    &button_x,
                    &button_y,
                );
            }
            if !suggested.is_empty() {
                for (binding_label, binding_path, binding) in suggested {
                    suggest_binding_best_effort(
                        instance,
                        profile,
                        spec.profile,
                        binding_label,
                        binding_path,
                        binding,
                    )?;
                }
            }
        }

        session
            .attach_action_sets(&[&action_set])
            .map_err(|e| format!("attach_action_sets: {e:?}"))?;

        Ok(ControllerInput {
            action_set,
            aim_pose,
            grip_pose,
            select,
            left_stick_x,
            left_stick_y,
            right_stick_x,
            right_stick_y,
            trigger_value,
            trigger_click,
            grip_value,
            grip_click,
            button_a,
            button_b,
            button_x,
            button_y,
            left,
            right,
            left_aim_space,
            right_aim_space,
            left_grip_space,
            right_grip_space,
            profile_poll_counter: 0,
            last_logged_left_profile: None,
            last_logged_right_profile: None,
            last_debug_snapshot: None,
        })
    }

    fn first_enabled_camera_xr(world: &World) -> Option<ComponentId> {
        world
            .all_components()
            .filter_map(|id| {
                world
                    .get_component_by_id_as::<CameraXRComponent>(id)
                    .map(|c| (id, c.enabled))
            })
            .find(|(_, enabled)| *enabled)
            .map(|(id, _)| id)
    }

    fn mul_mat4(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
        let mut c = [[0.0f32; 4]; 4];
        for col in 0..4 {
            for row in 0..4 {
                c[col][row] = a[0][row] * b[col][0]
                    + a[1][row] * b[col][1]
                    + a[2][row] * b[col][2]
                    + a[3][row] * b[col][3];
            }
        }
        c
    }

    fn transform_from_matrix_world(
        m: [[f32; 4]; 4],
    ) -> crate::engine::graphics::primitives::Transform {
        let mut t = crate::engine::graphics::primitives::Transform::default();
        t.model = m;
        t.matrix_world = m;
        t
    }

    fn mat4_from_pose(pose: openxr::Posef) -> [[f32; 4]; 4] {
        // IMPORTANT: This must match the engine's quaternion convention.
        // `Transform::recompute_model` is the canonical implementation.
        let q = pose.orientation;
        let p = pose.position;

        let mut t = crate::engine::graphics::primitives::Transform::default();
        t.translation = [p.x, p.y, p.z];
        t.rotation = [q.x, q.y, q.z, q.w];
        t.scale = [1.0, 1.0, 1.0];
        t.recompute_model();

        // OpenXR view poses are right-handed with -Z forward, which matches the engine's
        // Camera3D convention (forward -Z) and our projection math.
        //
        // Do not apply additional basis flips here unless we have a proven convention mismatch,
        // since flipping X/Z will also flip head translation direction and can break stereo.
        t.model
    }

    fn invert_affine_transform(m: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
        // Upper-left 3x3 in column-major.
        let c0 = [m[0][0], m[0][1], m[0][2]];
        let c1 = [m[1][0], m[1][1], m[1][2]];
        let c2 = [m[2][0], m[2][1], m[2][2]];

        // Row-major elements for determinant/cofactors.
        let a00 = c0[0];
        let a10 = c0[1];
        let a20 = c0[2];
        let a01 = c1[0];
        let a11 = c1[1];
        let a21 = c1[2];
        let a02 = c2[0];
        let a12 = c2[1];
        let a22 = c2[2];

        let det = a00 * (a11 * a22 - a12 * a21) - a01 * (a10 * a22 - a12 * a20)
            + a02 * (a10 * a21 - a11 * a20);

        if det.abs() < 1e-8 {
            return [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ];
        }
        let inv_det = 1.0 / det;

        // Inverse in row-major.
        let inv00 = (a11 * a22 - a12 * a21) * inv_det;
        let inv01 = (a02 * a21 - a01 * a22) * inv_det;
        let inv02 = (a01 * a12 - a02 * a11) * inv_det;

        let inv10 = (a12 * a20 - a10 * a22) * inv_det;
        let inv11 = (a00 * a22 - a02 * a20) * inv_det;
        let inv12 = (a02 * a10 - a00 * a12) * inv_det;

        let inv20 = (a10 * a21 - a11 * a20) * inv_det;
        let inv21 = (a01 * a20 - a00 * a21) * inv_det;
        let inv22 = (a00 * a11 - a01 * a10) * inv_det;

        let tx = m[3][0];
        let ty = m[3][1];
        let tz = m[3][2];

        let itx = -(inv00 * tx + inv01 * ty + inv02 * tz);
        let ity = -(inv10 * tx + inv11 * ty + inv12 * tz);
        let itz = -(inv20 * tx + inv21 * ty + inv22 * tz);

        [
            [inv00, inv10, inv20, 0.0],
            [inv01, inv11, inv21, 0.0],
            [inv02, inv12, inv22, 0.0],
            [itx, ity, itz, 1.0],
        ]
    }

    fn proj_from_fov_rh_zo(fov: openxr::Fovf, z_near: f32, z_far: f32) -> [[f32; 4]; 4] {
        let l = (fov.angle_left).tan() * z_near;
        let r = (fov.angle_right).tan() * z_near;
        let d = (fov.angle_down).tan() * z_near;
        let u = (fov.angle_up).tan() * z_near;

        let w = r - l;
        let h = u - d;
        let nf = 1.0 / (z_near - z_far);

        // Match the engine's column-major RH ZO layout used in Camera3D::perspective_rh_zo.
        // Now: +Y is up in clip space, matching OpenXR and GLTF conventions.
        [
            [2.0 * z_near / w, 0.0, 0.0, 0.0],
            [0.0, 2.0 * z_near / h, 0.0, 0.0],
            [(r + l) / w, (u + d) / h, z_far * nf, -1.0],
            [0.0, 0.0, (z_near * z_far) * nf, 0.0],
        ]
    }
}

impl System for OpenXRSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        self.pump_events();
    }
}

impl VrBackend for OpenXRSystem {
    fn kind(&self) -> VrBackendKind {
        VrBackendKind::OpenXR
    }

    fn initialize_runtime(&mut self) -> Result<(), String> {
        OpenXRSystem::initialize_runtime(self)
    }

    fn last_init_error(&self) -> Option<&str> {
        OpenXRSystem::last_init_error(self)
    }

    fn xr_input_state(&self) -> &XrInputState {
        OpenXRSystem::xr_input_state(self)
    }

    fn xr_gamepad_state(&self) -> &XrGamepadState {
        OpenXRSystem::xr_gamepad_state(self)
    }

    fn set_preferred_swapchain_format(&mut self, format: u32) {
        OpenXRSystem::set_preferred_swapchain_format(self, format)
    }

    fn required_vulkan_extensions(&self) -> Option<(Vec<String>, Vec<String>)> {
        OpenXRSystem::required_vulkan_extensions(self)
    }

    fn set_vulkan_graphics(&mut self, gfx: XrVulkanGraphics) {
        OpenXRSystem::set_vulkan_graphics(self, gfx)
    }

    fn register_vr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        OpenXRSystem::register_vr(self, world, visuals, component)
    }

    fn register_controller_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        OpenXRSystem::register_controller_xr(self, world, visuals, component)
    }

    fn register_input_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        OpenXRSystem::register_input_xr(self, world, visuals, component)
    }

    fn remove_controller_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        OpenXRSystem::remove_controller_xr(self, world, visuals, component)
    }

    fn remove_input_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        OpenXRSystem::remove_input_xr(self, world, visuals, component)
    }

    fn tick_with_queue(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        input: &InputState,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        dt_sec: f32,
    ) {
        OpenXRSystem::tick_with_queue(self, world, visuals, input, emit, dt_sec)
    }

    fn last_render_dt_sec(&self) -> Option<f32> {
        OpenXRSystem::last_render_dt_sec(self)
    }

    fn render_xr(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        renderer: &mut VulkanoRenderer,
    ) {
        OpenXRSystem::render_xr(self, world, visuals, renderer)
    }

    fn tick(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        input: &InputState,
        dt_sec: f32,
    ) {
        System::tick(self, world, visuals, input, dt_sec)
    }
}

impl OpenXRSystem {
    pub fn last_render_dt_sec(&self) -> Option<f32> {
        self.last_render_dt_sec
    }

    pub fn render_xr(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        renderer: &mut VulkanoRenderer,
    ) {
        let Some(state) = self.state.as_mut() else {
            self.xr_input_state = XrInputState::default();
            self.xr_gamepad_state = XrGamepadState::default();
            visuals.set_xr_frame_dt_sec(None);
            return;
        };

        let Some(sess) = state.session.as_mut() else {
            self.xr_input_state = XrInputState::default();
            self.xr_gamepad_state = XrGamepadState::default();
            visuals.set_xr_frame_dt_sec(None);
            return;
        };
        if !sess.running {
            self.xr_input_state = XrInputState::default();
            self.xr_gamepad_state = XrGamepadState::default();
            visuals.set_xr_frame_dt_sec(None);
            return;
        }

        let now = Instant::now();
        let dt_sec = self
            .last_render_instant
            .map(|prev| now.saturating_duration_since(prev).as_secs_f32());
        self.last_render_instant = Some(now);
        if let Some(dt_sec) = dt_sec {
            self.last_render_dt_sec = Some(dt_sec);
            visuals.set_xr_frame_dt_sec(Some(dt_sec));
        }

        // If no XR camera is enabled, do not enter the OpenXR frame pacing path at all.
        // `wait_frame()` can block to headset cadence, which would otherwise throttle the
        // desktop window even though XR scene rendering is effectively disabled.
        if visuals
            .active_xr_camera()
            .or_else(|| Self::first_enabled_camera_xr(world))
            .is_none()
        {
            self.xr_input_state = XrInputState::default();
            self.xr_gamepad_state = XrGamepadState::default();
            sess.head_pose_cache = None;
            sess.hand_root_pose_cache = HandRootPoseCache::default();
            sess.controller_pose_cache = ControllerPoseCache::default();
            visuals.set_xr_frame_dt_sec(None);
            return;
        }

        let frame_state = match sess.frame_waiter.wait() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[OpenXR] wait_frame failed: {e:?}");
                return;
            }
        };

        if let Err(e) = sess.frame_stream.begin() {
            eprintln!("[OpenXR] begin_frame failed: {e:?}");
            return;
        }

        if !frame_state.should_render {
            let _ =
                sess.frame_stream
                    .end(frame_state.predicted_display_time, state.blend_mode, &[]);
            return;
        }

        let views = match sess.session.locate_views(
            state.view_type,
            frame_state.predicted_display_time,
            &sess.reference_space,
        ) {
            Ok((_flags, views)) => views,
            Err(e) => {
                eprintln!("[OpenXR] locate_views failed: {e:?}");
                let _ = sess.frame_stream.end(
                    frame_state.predicted_display_time,
                    state.blend_mode,
                    &[],
                );
                return;
            }
        };

        sess.head_pose_cache = Self::derive_head_pose(&views);
        self.xr_gamepad_state.head_pose_rotation = sess.head_pose_cache.map(Self::quat_from_posef);

        let mut left_root_for_debug: Option<(openxr::Posef, openxr::HandJointEXT)> = None;
        let mut right_root_for_debug: Option<(openxr::Posef, openxr::HandJointEXT)> = None;
        let mut left_joints_for_debug: Option<openxr::HandJointLocations> = None;
        let mut right_joints_for_debug: Option<openxr::HandJointLocations> = None;

        if let Some(hand_tracking) = sess.hand_tracking.as_ref() {
            let left_joints = sess
                .reference_space
                .locate_hand_joints(&hand_tracking.left, frame_state.predicted_display_time);
            let right_joints = sess
                .reference_space
                .locate_hand_joints(&hand_tracking.right, frame_state.predicted_display_time);

            match left_joints {
                Ok(Some(joints)) => {
                    let root = Self::select_hand_root_pose(&joints);
                    sess.hand_root_pose_cache.left_root = root.map(|(pose, _)| pose);
                    sess.hand_root_pose_cache.left_root_joint = root.map(|(_, joint)| joint);
                    left_root_for_debug = root;
                    left_joints_for_debug = Some(joints);
                }
                Ok(None) => {
                    sess.hand_root_pose_cache.left_root = None;
                    sess.hand_root_pose_cache.left_root_joint = None;
                }
                Err(e) => {
                    sess.hand_root_pose_cache.left_root = None;
                    sess.hand_root_pose_cache.left_root_joint = None;
                    eprintln!("[OpenXR] locate_hand_joints(left) failed: {e:?}");
                }
            }

            match right_joints {
                Ok(Some(joints)) => {
                    let root = Self::select_hand_root_pose(&joints);
                    sess.hand_root_pose_cache.right_root = root.map(|(pose, _)| pose);
                    sess.hand_root_pose_cache.right_root_joint = root.map(|(_, joint)| joint);
                    right_root_for_debug = root;
                    right_joints_for_debug = Some(joints);
                }
                Ok(None) => {
                    sess.hand_root_pose_cache.right_root = None;
                    sess.hand_root_pose_cache.right_root_joint = None;
                }
                Err(e) => {
                    sess.hand_root_pose_cache.right_root = None;
                    sess.hand_root_pose_cache.right_root_joint = None;
                    eprintln!("[OpenXR] locate_hand_joints(right) failed: {e:?}");
                }
            }
        } else {
            sess.hand_root_pose_cache = HandRootPoseCache::default();
        }

        Self::log_hand_debug_snapshot(
            sess,
            left_joints_for_debug.as_ref(),
            right_joints_for_debug.as_ref(),
            sess.hand_root_pose_cache.left_root_joint,
            sess.hand_root_pose_cache.right_root_joint,
        );

        if let Some((pose, joint)) = left_root_for_debug {
            Self::update_hand_rotation_debug(
                &mut sess.hand_rotation_debug,
                ControllerHand::Left,
                Some(joint),
                pose,
            );
        }
        if let Some((pose, joint)) = right_root_for_debug {
            Self::update_hand_rotation_debug(
                &mut sess.hand_rotation_debug,
                ControllerHand::Right,
                Some(joint),
                pose,
            );
        }

        // Update controller pose cache at the same predicted time as views.
        if let Some(ci) = sess.controller_input.as_mut() {
            // Sync actions (best-effort).
            let active = openxr::ActiveActionSet::new(&ci.action_set);
            match sess.session.sync_actions(&[active]) {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("[OpenXR] sync_actions failed: {e:?}");
                }
            }
            let should_poll_profiles =
                ci.profile_poll_counter == 0 || ci.profile_poll_counter >= 89;
            ci.profile_poll_counter = if should_poll_profiles {
                1
            } else {
                ci.profile_poll_counter.saturating_add(1)
            };
            if should_poll_profiles {
                log_active_interaction_profile(
                    &state.instance,
                    &sess.session,
                    ci.left,
                    "/user/hand/left",
                    &mut ci.last_logged_left_profile,
                );
                log_active_interaction_profile(
                    &state.instance,
                    &sess.session,
                    ci.right,
                    "/user/hand/right",
                    &mut ci.last_logged_right_profile,
                );
            }
            let update_pose = |space: &openxr::Space, base: &openxr::Space, t: openxr::Time| {
                space.locate(base, t).ok()
            };

            let left_profile = ci.last_logged_left_profile;
            let right_profile = ci.last_logged_right_profile;

            sess.controller_pose_cache.left_aim = update_pose(
                &ci.left_aim_space,
                &sess.reference_space,
                frame_state.predicted_display_time,
            )
            .filter(|loc| {
                loc.location_flags
                    .contains(openxr::SpaceLocationFlags::POSITION_VALID)
                    && loc
                        .location_flags
                        .contains(openxr::SpaceLocationFlags::ORIENTATION_VALID)
            })
            .map(|loc| loc.pose);
            sess.controller_pose_cache.right_aim = update_pose(
                &ci.right_aim_space,
                &sess.reference_space,
                frame_state.predicted_display_time,
            )
            .filter(|loc| {
                loc.location_flags
                    .contains(openxr::SpaceLocationFlags::POSITION_VALID)
                    && loc
                        .location_flags
                        .contains(openxr::SpaceLocationFlags::ORIENTATION_VALID)
            })
            .map(|loc| loc.pose);

            sess.controller_pose_cache.left_grip = update_pose(
                &ci.left_grip_space,
                &sess.reference_space,
                frame_state.predicted_display_time,
            )
            .filter(|loc| {
                loc.location_flags
                    .contains(openxr::SpaceLocationFlags::POSITION_VALID)
                    && loc
                        .location_flags
                        .contains(openxr::SpaceLocationFlags::ORIENTATION_VALID)
            })
            .map(|loc| loc.pose);
            sess.controller_pose_cache.right_grip = update_pose(
                &ci.right_grip_space,
                &sess.reference_space,
                frame_state.predicted_display_time,
            )
            .filter(|loc| {
                loc.location_flags
                    .contains(openxr::SpaceLocationFlags::POSITION_VALID)
                    && loc
                        .location_flags
                        .contains(openxr::SpaceLocationFlags::ORIENTATION_VALID)
            })
            .map(|loc| loc.pose);

            let left_select = ci.select.state(&sess.session, ci.left).ok();
            let right_select = ci.select.state(&sess.session, ci.right).ok();
            let left_stick_x = ci.left_stick_x.state(&sess.session, ci.left).ok();
            let left_stick_y = ci.left_stick_y.state(&sess.session, ci.left).ok();
            let right_stick_x = ci.right_stick_x.state(&sess.session, ci.right).ok();
            let right_stick_y = ci.right_stick_y.state(&sess.session, ci.right).ok();
            let left_trigger_value = ci.trigger_value.state(&sess.session, ci.left).ok();
            let right_trigger_value = ci.trigger_value.state(&sess.session, ci.right).ok();
            let left_trigger_click = ci.trigger_click.state(&sess.session, ci.left).ok();
            let right_trigger_click = ci.trigger_click.state(&sess.session, ci.right).ok();
            let left_grip_value = ci.grip_value.state(&sess.session, ci.left).ok();
            let right_grip_value = ci.grip_value.state(&sess.session, ci.right).ok();
            let left_grip_click = ci.grip_click.state(&sess.session, ci.left).ok();
            let right_grip_click = ci.grip_click.state(&sess.session, ci.right).ok();
            let left_x = ci.button_x.state(&sess.session, ci.left).ok();
            let left_y = ci.button_y.state(&sess.session, ci.left).ok();
            let right_a = ci.button_a.state(&sess.session, ci.right).ok();
            let right_b = ci.button_b.state(&sess.session, ci.right).ok();
            let snapshot = ControllerDebugSnapshot {
                left_profile: profile_string_or_none(&state.instance, left_profile),
                right_profile: profile_string_or_none(&state.instance, right_profile),
                left_aim_valid: sess.controller_pose_cache.left_aim.is_some(),
                right_aim_valid: sess.controller_pose_cache.right_aim.is_some(),
                left_grip_valid: sess.controller_pose_cache.left_grip.is_some(),
                right_grip_valid: sess.controller_pose_cache.right_grip.is_some(),
                left_select: left_select.map(|s| (s.is_active, s.current_state)),
                right_select: right_select.map(|s| (s.is_active, s.current_state)),
                left_thumbstick: scalar_stick_state(left_stick_x, left_stick_y),
                right_thumbstick: scalar_stick_state(right_stick_x, right_stick_y),
                left_trigger_value: left_trigger_value.map(|s| (s.is_active, s.current_state)),
                right_trigger_value: right_trigger_value.map(|s| (s.is_active, s.current_state)),
                left_trigger_click: left_trigger_click.map(|s| (s.is_active, s.current_state)),
                right_trigger_click: right_trigger_click.map(|s| (s.is_active, s.current_state)),
                left_grip_value: left_grip_value.map(|s| (s.is_active, s.current_state)),
                right_grip_value: right_grip_value.map(|s| (s.is_active, s.current_state)),
                left_grip_click: left_grip_click.map(|s| (s.is_active, s.current_state)),
                right_grip_click: right_grip_click.map(|s| (s.is_active, s.current_state)),
                left_x: left_x.map(|s| (s.is_active, s.current_state)),
                left_y: left_y.map(|s| (s.is_active, s.current_state)),
                right_a: right_a.map(|s| (s.is_active, s.current_state)),
                right_b: right_b.map(|s| (s.is_active, s.current_state)),
            };
            if openxr_debug_enabled() && ci.last_debug_snapshot.as_ref() != Some(&snapshot) {
                eprintln!(
                    "[OpenXR][debug] profiles left={} right={}",
                    snapshot.left_profile, snapshot.right_profile,
                );
                eprintln!(
                    "[OpenXR][debug] pose_valid left_aim={} right_aim={} left_grip={} right_grip={}",
                    snapshot.left_aim_valid,
                    snapshot.right_aim_valid,
                    snapshot.left_grip_valid,
                    snapshot.right_grip_valid,
                );
                eprintln!(
                    "[OpenXR][debug] select L={:?} R={:?} thumbstick L={:?} R={:?}",
                    snapshot.left_select,
                    snapshot.right_select,
                    snapshot.left_thumbstick,
                    snapshot.right_thumbstick,
                );
                eprintln!(
                    "[OpenXR][debug] trigger_value L={:?} R={:?} trigger_click L={:?} R={:?}",
                    snapshot.left_trigger_value,
                    snapshot.right_trigger_value,
                    snapshot.left_trigger_click,
                    snapshot.right_trigger_click,
                );
                eprintln!(
                    "[OpenXR][debug] grip_value L={:?} R={:?} grip_click L={:?} R={:?}",
                    snapshot.left_grip_value,
                    snapshot.right_grip_value,
                    snapshot.left_grip_click,
                    snapshot.right_grip_click,
                );
                eprintln!(
                    "[OpenXR][debug] face_buttons X={:?} Y={:?} A={:?} B={:?}",
                    snapshot.left_x,
                    snapshot.left_y,
                    snapshot.right_a,
                    snapshot.right_b,
                );
            }
            ci.last_debug_snapshot = Some(snapshot);

            // Poll select action for each hand and derive pressed/down/released edges.
            let prev = self.xr_input_state.trigger_down;
            self.xr_gamepad_state = XrGamepadState {
                active: true,
                hands: [XrHandGamepadState::default(), XrHandGamepadState::default()],
                head_pose_rotation: self.xr_gamepad_state.head_pose_rotation,
            };
            for i in 0..2 {
                let hand_path = if i == 0 { ci.left } else { ci.right };
                let cur_down = ci
                    .select
                    .state(&sess.session, hand_path)
                    .ok()
                    .map(|s| s.current_state)
                    .unwrap_or(false);
                self.xr_input_state.trigger_down[i] = cur_down;
                self.xr_input_state.trigger_pressed[i] = cur_down && !prev[i];
                self.xr_input_state.trigger_released[i] = !cur_down && prev[i];

                self.xr_gamepad_state.hands[i].thumbstick = match i {
                    0 => scalar_stick_value(
                        ci.left_stick_x.state(&sess.session, ci.left).ok(),
                        ci.left_stick_y.state(&sess.session, ci.left).ok(),
                    ),
                    1 => scalar_stick_value(
                        ci.right_stick_x.state(&sess.session, ci.right).ok(),
                        ci.right_stick_y.state(&sess.session, ci.right).ok(),
                    ),
                    _ => None,
                };

                let trigger_value = ci
                    .trigger_value
                    .state(&sess.session, hand_path)
                    .ok()
                    .filter(|s| s.is_active)
                    .map(|s| s.current_state);
                let trigger_click = ci
                    .trigger_click
                    .state(&sess.session, hand_path)
                    .ok()
                    .filter(|s| s.is_active)
                    .map(|s| s.current_state);
                self.xr_gamepad_state.hands[i].trigger_value = trigger_value;
                self.xr_gamepad_state.hands[i].trigger_pressed = trigger_click
                    .map(|down| (down, if down { 1.0 } else { 0.0 }))
                    .or_else(|| trigger_value.map(|value| (value >= 0.7, value)));

                let grip_value = ci
                    .grip_value
                    .state(&sess.session, hand_path)
                    .ok()
                    .filter(|s| s.is_active)
                    .map(|s| s.current_state);
                let grip_click = ci
                    .grip_click
                    .state(&sess.session, hand_path)
                    .ok()
                    .filter(|s| s.is_active)
                    .map(|s| s.current_state);
                self.xr_gamepad_state.hands[i].grip_value = grip_value;
                self.xr_gamepad_state.hands[i].grip_pressed = grip_click
                    .map(|down| (down, if down { 1.0 } else { 0.0 }))
                    .or_else(|| grip_value.map(|value| (value >= 0.7, value)));
            }
            self.xr_gamepad_state.hands[0].button_x = ci
                .button_x
                .state(&sess.session, ci.left)
                .ok()
                .filter(|s| s.is_active)
                .map(|s| (s.current_state, if s.current_state { 1.0 } else { 0.0 }));
            self.xr_gamepad_state.hands[0].button_y = ci
                .button_y
                .state(&sess.session, ci.left)
                .ok()
                .filter(|s| s.is_active)
                .map(|s| (s.current_state, if s.current_state { 1.0 } else { 0.0 }));
            self.xr_gamepad_state.hands[1].button_a = ci
                .button_a
                .state(&sess.session, ci.right)
                .ok()
                .filter(|s| s.is_active)
                .map(|s| (s.current_state, if s.current_state { 1.0 } else { 0.0 }));
            self.xr_gamepad_state.hands[1].button_b = ci
                .button_b
                .state(&sess.session, ci.right)
                .ok()
                .filter(|s| s.is_active)
                .map(|s| (s.current_state, if s.current_state { 1.0 } else { 0.0 }));
            log_xr_gamepad_changes(self.xr_gamepad_state_last_logged, self.xr_gamepad_state);
            self.xr_gamepad_state_last_logged = self.xr_gamepad_state;
        } else {
            self.xr_input_state = XrInputState::default();
            self.xr_gamepad_state = XrGamepadState::default();
            log_xr_gamepad_changes(self.xr_gamepad_state_last_logged, self.xr_gamepad_state);
            self.xr_gamepad_state_last_logged = self.xr_gamepad_state;
        }

        // Publish XR per-eye camera matrices into VisualWorld (CameraTarget::Xr).
        let rig_world = Self::xr_rig_origin_world(world, visuals);

        let mut eyes = Vec::with_capacity(views.len());
        for v in &views {
            let eye_from_space = Self::mat4_from_pose(v.pose);
            let world_from_eye = Self::mul_mat4(&rig_world, &eye_from_space);
            let view = Self::invert_affine_transform(&world_from_eye);
            let proj = Self::proj_from_fov_rh_zo(v.fov, 0.1, 100.0);
            eyes.push(CameraData {
                view,
                proj,
                transform: Self::transform_from_matrix_world(world_from_eye),
            });
        }
        visuals.set_xr_camera(eyes);

        // Acquire XR swapchain image.
        let image_index = {
            let swapchain = sess.xr_swapchain.swapchain_mut();
            match swapchain.acquire_image() {
                Ok(i) => i,
                Err(e) => {
                    eprintln!("[OpenXR] acquire_image failed: {e:?}");
                    let _ = sess.frame_stream.end(
                        frame_state.predicted_display_time,
                        state.blend_mode,
                        &[],
                    );
                    return;
                }
            }
        };

        if let Err(e) = sess
            .xr_swapchain
            .swapchain_mut()
            .wait_image(openxr::Duration::INFINITE)
        {
            eprintln!("[OpenXR] wait_image failed: {e:?}");
        } else {
            // Render into offscreen Vulkano images (per eye), then copy into the OpenXR swapchain layers.
            let extent = sess.xr_swapchain.extent();
            let extent_u = [extent.width as u32, extent.height as u32];

            let image_index_usize = image_index as usize;
            let dst_was_initialized = sess
                .swapchain_image_initialized
                .get(image_index_usize)
                .copied()
                .unwrap_or(false);

            // Render both eyes first (blocking bring-up path).
            let view_count = sess.xr_swapchain.view_count() as usize;

            let xr_format = sess.xr_swapchain.format();
            let window_format = renderer.window_vk_format_raw();
            let format_matches = window_format.map(|f| f == xr_format).unwrap_or(true);

            // Copy requires compatible formats. If we couldn't match formats at swapchain creation,
            // fall back to a pink clear to prove we're submitting frames.
            let dst_image = sess.xr_swapchain.images()[image_index_usize];
            if !format_matches {
                if !sess.did_log_format_mismatch {
                    sess.did_log_format_mismatch = true;
                    eprintln!(
                        "[OpenXR] XR swapchain format mismatch (xr={} window={:?}); falling back to clear_color",
                        xr_format, window_format
                    );
                }

                if let Err(e) = xr_renderer::clear_xr_swapchain_image(
                    &sess.vk_device,
                    sess.vk_queue,
                    sess.vk_command_buffer,
                    sess.xr_swapchain.view_count(),
                    dst_image,
                    visuals.clear_color(),
                    dst_was_initialized,
                ) {
                    eprintln!("[OpenXR] clear XR image failed: {e:?}");
                } else if let Some(slot) =
                    sess.swapchain_image_initialized.get_mut(image_index_usize)
                {
                    *slot = true;
                }
            } else {
                for eye in 0..view_count.min(views.len()) {
                    if let Err(e) = renderer.render_xr_eye_offscreen(visuals, eye, extent_u) {
                        eprintln!("[OpenXR] render_xr_eye_offscreen failed: {e}");
                    }
                }

                if let Err(e) = xr_renderer::copy_offscreen_to_xr_layers(
                    &sess.vk_device,
                    sess.vk_queue,
                    sess.vk_command_buffer,
                    &sess.xr_swapchain,
                    renderer,
                    dst_image,
                    dst_was_initialized,
                    view_count,
                ) {
                    eprintln!("[OpenXR] copy to XR image failed: {e:?}");
                } else if let Some(slot) =
                    sess.swapchain_image_initialized.get_mut(image_index_usize)
                {
                    *slot = true;
                }
            }
        }

        if let Err(e) = sess.xr_swapchain.swapchain_mut().release_image() {
            eprintln!("[OpenXR] release_image failed: {e:?}");
        }

        // Submit a projection layer.
        if views.len() >= 2 {
            let rect = openxr::Rect2Di {
                offset: openxr::Offset2Di { x: 0, y: 0 },
                extent: sess.xr_swapchain.extent(),
            };

            let pv0 = openxr::CompositionLayerProjectionView::new()
                .pose(views[0].pose)
                .fov(views[0].fov)
                .sub_image(
                    openxr::SwapchainSubImage::new()
                        .swapchain(sess.xr_swapchain.swapchain())
                        .image_array_index(0)
                        .image_rect(rect),
                );

            let pv1 = openxr::CompositionLayerProjectionView::new()
                .pose(views[1].pose)
                .fov(views[1].fov)
                .sub_image(
                    openxr::SwapchainSubImage::new()
                        .swapchain(sess.xr_swapchain.swapchain())
                        .image_array_index(1)
                        .image_rect(rect),
                );

            let projection_views = [pv0, pv1];
            let layer = openxr::CompositionLayerProjection::new()
                .space(&sess.reference_space)
                .views(&projection_views);

            if let Err(e) = sess.frame_stream.end(
                frame_state.predicted_display_time,
                state.blend_mode,
                &[&layer],
            ) {
                eprintln!("[OpenXR] end_frame failed: {e:?}");
            }

            return;
        }

        if let Err(e) =
            sess.frame_stream
                .end(frame_state.predicted_display_time, state.blend_mode, &[])
        {
            eprintln!("[OpenXR] end_frame failed: {e:?}");
        }
    }
}
