use crate::engine::ecs::component::{
    ControllerHand, InputXRComponent, InputXRGamepadComponent, TransformComponent, XrAxisControl,
    XrButtonControl, XrHandPreference,
};
use crate::engine::ecs::system::{System, TransformSystem, XrGamepadState, XrSystem};
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, World};
use crate::engine::graphics::{CameraTarget, VisualWorld};
use crate::engine::user_input::InputState;
use crate::utils::math;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

fn xr_input_event_log_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CAT_XR_INPUT_LOG")
            .ok()
            .map(|s| {
                let s = s.trim().to_ascii_lowercase();
                s == "1" || s == "true" || s == "on" || s == "yes"
            })
            .unwrap_or(false)
    })
}

#[derive(Debug, Clone, Default)]
struct ComponentEventState {
    buttons: HashMap<(ControllerHand, XrButtonControl), bool>,
    axes: HashMap<(ControllerHand, XrAxisControl), [f32; 2]>,
}

#[derive(Debug, Default)]
pub struct InputXRGamepadSystem {
    components: HashSet<ComponentId>,
    previous: HashMap<ComponentId, ComponentEventState>,
    xr_active_last_frame: bool,
    logged_missing_owner: HashSet<ComponentId>,
    logged_missing_locomotion_target: HashSet<ComponentId>,
}

impl InputXRGamepadSystem {
    pub fn register_input_xr_gamepad(&mut self, component: ComponentId) {
        self.components.insert(component);
    }

    pub fn remove_input_xr_gamepad(&mut self, component: ComponentId) {
        self.components.remove(&component);
        self.previous.remove(&component);
        self.logged_missing_owner.remove(&component);
        self.logged_missing_locomotion_target.remove(&component);
    }

    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        xr: &XrSystem,
        emit: &mut dyn SignalEmitter,
        dt_sec: f32,
    ) {
        let xr_state = *xr.xr_gamepad_state();
        if !xr_state.active {
            if self.xr_active_last_frame {
                eprintln!("[input_xr_gamepad_system] XR gamepad input inactive");
            }
            self.xr_active_last_frame = false;
            return;
        }
        if !self.xr_active_last_frame {
            eprintln!("[input_xr_gamepad_system] XR gamepad input active");
        }
        self.xr_active_last_frame = true;

        let components: Vec<_> = self.components.iter().copied().collect();
        for cid in components {
            let Some(cfg) = world
                .get_component_by_id_as::<InputXRGamepadComponent>(cid)
                .cloned()
            else {
                self.remove_input_xr_gamepad(cid);
                continue;
            };
            if !cfg.enabled {
                continue;
            }

            let Some(input_xr_owner) = nearest_ancestor_component::<InputXRComponent>(world, cid)
            else {
                if self.logged_missing_owner.insert(cid) {
                    eprintln!(
                        "[input_xr_gamepad_system] component {:?} has no owning InputXR ancestor; skipping XR input",
                        cid
                    );
                }
                continue;
            };
            self.logged_missing_owner.remove(&cid);

            let prev = self.previous.get(&cid).cloned().unwrap_or_default();
            let mut next = ComponentEventState::default();
            emit_axis_events(cid, cfg.hand, &xr_state, &prev, &mut next, emit);
            emit_button_events(cid, cfg.hand, &xr_state, &prev, &mut next, emit);
            self.previous.insert(cid, next);

            if cfg.locomotion {
                let moved = apply_locomotion(
                    world,
                    visuals,
                    input_xr_owner,
                    &cfg,
                    &xr_state,
                    emit,
                    dt_sec,
                );
                if moved {
                    self.logged_missing_locomotion_target.remove(&cid);
                } else if xr_locomotion_target_transform(world, input_xr_owner).is_none()
                    && self.logged_missing_locomotion_target.insert(cid)
                {
                    eprintln!(
                        "[input_xr_gamepad_system] component {:?} has locomotion enabled but no transform ancestor above InputXR",
                        cid
                    );
                }
            }
        }
    }
}

impl System for InputXRGamepadSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
    }
}

fn emit_axis_events(
    cid: ComponentId,
    hand_pref: XrHandPreference,
    xr: &XrGamepadState,
    prev: &ComponentEventState,
    next: &mut ComponentEventState,
    emit: &mut dyn SignalEmitter,
) {
    for &(hand, control, value) in &[
        (
            ControllerHand::Left,
            XrAxisControl::LeftStick,
            xr.hands[0].thumbstick,
        ),
        (
            ControllerHand::Right,
            XrAxisControl::RightStick,
            xr.hands[1].thumbstick,
        ),
        (
            ControllerHand::Left,
            XrAxisControl::LeftTrigger,
            xr.hands[0].trigger_value.map(|v| [v, 0.0]),
        ),
        (
            ControllerHand::Right,
            XrAxisControl::RightTrigger,
            xr.hands[1].trigger_value.map(|v| [v, 0.0]),
        ),
        (
            ControllerHand::Left,
            XrAxisControl::LeftGrip,
            xr.hands[0].grip_value.map(|v| [v, 0.0]),
        ),
        (
            ControllerHand::Right,
            XrAxisControl::RightGrip,
            xr.hands[1].grip_value.map(|v| [v, 0.0]),
        ),
    ] {
        if !hand_matches(hand_pref, hand) {
            continue;
        }
        let Some(value) = value else {
            continue;
        };
        next.axes.insert((hand, control), value);
        if prev.axes.get(&(hand, control)).copied() != Some(value) {
            if xr_input_event_log_enabled() {
                eprintln!(
                    "[input_xr_gamepad_system] axis {:?} {:?} -> [{:.3}, {:.3}] on {:?}",
                    hand, control, value[0], value[1], cid
                );
            }
            emit.push_event(
                cid,
                EventSignal::XrAxisChanged {
                    source_component: cid,
                    hand,
                    control,
                    value,
                },
            );
        }
    }
}

fn emit_button_events(
    cid: ComponentId,
    hand_pref: XrHandPreference,
    xr: &XrGamepadState,
    prev: &ComponentEventState,
    next: &mut ComponentEventState,
    emit: &mut dyn SignalEmitter,
) {
    for &(hand, control, state) in &[
        (
            ControllerHand::Left,
            XrButtonControl::LeftTrigger,
            xr.hands[0].trigger_pressed,
        ),
        (
            ControllerHand::Right,
            XrButtonControl::RightTrigger,
            xr.hands[1].trigger_pressed,
        ),
        (
            ControllerHand::Left,
            XrButtonControl::LeftGrip,
            xr.hands[0].grip_pressed,
        ),
        (
            ControllerHand::Right,
            XrButtonControl::RightGrip,
            xr.hands[1].grip_pressed,
        ),
        (
            ControllerHand::Left,
            XrButtonControl::ButtonX,
            xr.hands[0].button_x,
        ),
        (
            ControllerHand::Left,
            XrButtonControl::ButtonY,
            xr.hands[0].button_y,
        ),
        (
            ControllerHand::Right,
            XrButtonControl::ButtonA,
            xr.hands[1].button_a,
        ),
        (
            ControllerHand::Right,
            XrButtonControl::ButtonB,
            xr.hands[1].button_b,
        ),
    ] {
        if !hand_matches(hand_pref, hand) {
            continue;
        }
        let Some((down, value)) = state else {
            continue;
        };
        next.buttons.insert((hand, control), down);
        let was_down = prev.buttons.get(&(hand, control)).copied().unwrap_or(false);
        if was_down != down {
            if xr_input_event_log_enabled() {
                eprintln!(
                    "[input_xr_gamepad_system] button {:?} {:?} -> {} ({:.3}) on {:?}",
                    hand,
                    control,
                    if down { "down" } else { "up" },
                    value,
                    cid
                );
            }
            emit.push_event(
                cid,
                if down {
                    EventSignal::XrButtonDown {
                        source_component: cid,
                        hand,
                        control,
                        value,
                    }
                } else {
                    EventSignal::XrButtonUp {
                        source_component: cid,
                        hand,
                        control,
                        value,
                    }
                },
            );
        }
        if was_down != down {
            emit.push_event(
                cid,
                EventSignal::XrButtonChanged {
                    source_component: cid,
                    hand,
                    control,
                    value,
                },
            );
        }
    }
}

fn apply_locomotion(
    world: &mut World,
    visuals: &VisualWorld,
    input_xr_owner: ComponentId,
    cfg: &InputXRGamepadComponent,
    xr: &XrGamepadState,
    emit: &mut dyn SignalEmitter,
    dt_sec: f32,
) -> bool {
    let Some((_, stick)) = resolve_locomotion_stick(cfg.hand, xr) else {
        return false;
    };

    let mut dx = stick[0];
    let mut dz = -stick[1];
    let len = (dx * dx + dz * dz).sqrt();
    if len <= cfg.deadzone || len <= f32::EPSILON {
        return false;
    }
    let scaled = ((len - cfg.deadzone) / (1.0 - cfg.deadzone)).clamp(0.0, 1.0);
    dx = dx / len * scaled;
    dz = dz / len * scaled;

    let Some(target_tcid) = xr_locomotion_target_transform(world, input_xr_owner) else {
        return false;
    };

    // OpenXR has already published the final world-space eye transforms for the
    // active CameraXR this frame. Use that authoritative rendered orientation
    // directly; reconstructing it from the ECS hierarchy can double AVC yaw.
    let camera_world = visuals
        .visual_camera(CameraTarget::Xr)
        .and_then(|camera| camera.eyes.first())
        .map(|eye| eye.transform.matrix_world)
        .or_else(|| TransformSystem::world_model(world, target_tcid));
    let (right, back) = camera_world
        .and_then(horizontal_camera_basis)
        .unwrap_or(([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]));
    let move_world = [
        right[0] * dx + back[0] * dz,
        0.0,
        right[2] * dx + back[2] * dz,
    ];

    // The locomotion target stores a local translation. Convert the world-space
    // direction back through its parent so rotated parent rigs don't skew it.
    let move_local = world
        .parent_of(target_tcid)
        .and_then(|parent| TransformSystem::world_model(world, parent))
        .map(|parent_world| {
            math::quat_rotate_vec3(
                math::quat_conjugate(math::mat_to_quat(parent_world)),
                move_world,
            )
        })
        .unwrap_or(move_world);
    let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(target_tcid) else {
        return false;
    };
    t.transform.translation[0] += move_local[0] * cfg.speed * dt_sec;
    t.transform.translation[2] += move_local[2] * cfg.speed * dt_sec;
    t.transform.recompute_model();

    let transform = t.transform;
    emit.push_intent_now(
        target_tcid,
        IntentValue::UpdateTransform {
            component_ids: vec![target_tcid],
            translation: transform.translation,
            rotation_quat_xyzw: transform.rotation,
            scale: transform.scale,
        },
    );
    true
}

fn hand_matches(pref: XrHandPreference, hand: ControllerHand) -> bool {
    match pref {
        XrHandPreference::Default | XrHandPreference::Either => true,
        XrHandPreference::Left => hand == ControllerHand::Left,
        XrHandPreference::Right => hand == ControllerHand::Right,
    }
}

fn nearest_ancestor_component<T: 'static>(
    world: &World,
    start: ComponentId,
) -> Option<ComponentId> {
    let mut cur = world.parent_of(start);
    while let Some(cid) = cur {
        if world.get_component_by_id_as::<T>(cid).is_some() {
            return Some(cid);
        }
        cur = world.parent_of(cid);
    }
    None
}

pub(crate) fn xr_locomotion_target_transform(
    world: &World,
    input_xr_owner: ComponentId,
) -> Option<ComponentId> {
    let mut cur = world.parent_of(input_xr_owner);
    while let Some(cid) = cur {
        if world
            .get_component_by_id_as::<TransformComponent>(cid)
            .is_some()
        {
            return Some(cid);
        }
        cur = world.parent_of(cid);
    }
    None
}

fn resolve_locomotion_stick(
    pref: XrHandPreference,
    xr: &XrGamepadState,
) -> Option<(ControllerHand, [f32; 2])> {
    match pref {
        XrHandPreference::Left => xr.hands[0].thumbstick.map(|v| (ControllerHand::Left, v)),
        XrHandPreference::Right => xr.hands[1].thumbstick.map(|v| (ControllerHand::Right, v)),
        XrHandPreference::Either | XrHandPreference::Default => xr.hands[0]
            .thumbstick
            .map(|v| (ControllerHand::Left, v))
            .or_else(|| xr.hands[1].thumbstick.map(|v| (ControllerHand::Right, v))),
    }
}

fn horizontal_camera_basis(matrix_world: [[f32; 4]; 4]) -> Option<([f32; 3], [f32; 3])> {
    // Transform matrices are column-major: column 0 is camera-right and column
    // 2 is camera-back. Project both onto the ground plane and normalize them.
    let normalize_xz = |v: [f32; 3]| {
        let len = (v[0] * v[0] + v[2] * v[2]).sqrt();
        (len > 1e-6).then(|| [v[0] / len, 0.0, v[2] / len])
    };
    let right = normalize_xz([matrix_world[0][0], 0.0, matrix_world[0][2]])?;
    let back = normalize_xz([matrix_world[2][0], 0.0, matrix_world[2][2]])?;
    Some((right, back))
}
