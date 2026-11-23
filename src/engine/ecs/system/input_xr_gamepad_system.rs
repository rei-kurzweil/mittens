use crate::engine::ecs::component::{
    ControllerHand, InputXRComponent, InputXRGamepadComponent, TransformComponent, XrAxisControl,
    XrButtonControl, XrHandPreference, XrStickMode,
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
    flying: super::input_xr_flying::XrFlying,
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

    pub(crate) fn set_flight_enabled(&mut self, world: &mut World, id: ComponentId, enabled: bool) {
        self.flying.set_flight_enabled(world, id, enabled);
    }

    pub fn remove_input_xr_gamepad(&mut self, world: &mut World, component: ComponentId) {
        self.flying.stop(world, component);
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
        self.tick_with_state(world, visuals, *xr.xr_gamepad_state(), emit, dt_sec);
    }
    fn tick_with_state(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        xr_state: XrGamepadState,
        emit: &mut dyn SignalEmitter,
        dt_sec: f32,
    ) {
        self.flying.advance(dt_sec);
        if !xr_state.active {
            self.flying.suspend(world);
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
            let Some(mut cfg) = world
                .get_component_by_id_as::<InputXRGamepadComponent>(cid)
                .cloned()
            else {
                self.remove_input_xr_gamepad(world, cid);
                continue;
            };
            if !cfg.enabled {
                self.flying.stop(world, cid);
                continue;
            }

            let Some(input_xr_owner) = nearest_ancestor_component::<InputXRComponent>(world, cid)
            else {
                self.flying.stop(world, cid);
                if self.logged_missing_owner.insert(cid) {
                    eprintln!(
                        "[input_xr_gamepad_system] component {:?} has no owning InputXR ancestor; skipping XR input",
                        cid
                    );
                }
                continue;
            };
            self.logged_missing_owner.remove(&cid);

            if let Some(target) = xr_locomotion_target_transform(world, input_xr_owner) {
                cfg.flying = self.flying.update(world, cid, target, &xr_state);
            } else {
                self.flying.stop(world, cid);
            }
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
    if !dt_sec.is_finite() || dt_sec <= 0. {
        return false;
    }
    let (move_hand, stick) =
        resolve_locomotion_stick(cfg.hand, xr).unwrap_or((ControllerHand::Left, [0.; 2]));
    if !stick[0].is_finite() || !stick[1].is_finite() {
        return false;
    }
    let [dx, dz] = radial_deadzone([stick[0], -stick[1]], cfg.deadzone);
    let turn = match cfg.stick_mode {
        XrStickMode::Strafe => 0.,
        // The opposite hand's stick turns; only its X axis is read, with its
        // own deadzone so a slightly diagonal push does not creep the view.
        XrStickMode::Turn => {
            let turn_hand = match move_hand {
                ControllerHand::Left => 1,
                ControllerHand::Right => 0,
            };
            xr.hands[turn_hand]
                .thumbstick
                .filter(|v| v[0].is_finite())
                .map_or(0., |v| axis_deadzone(v[0], cfg.deadzone))
        }
    };
    let vertical = if cfg.flying {
        (xr.hands[1].button_b.is_some_and(|b| b.0) as u8 as f32)
            - (xr.hands[1].button_a.is_some_and(|b| b.0) as u8 as f32)
    } else {
        0.
    };
    if dx == 0. && dz == 0. && vertical == 0. && turn == 0. {
        return false;
    }

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
    let (right, back) = if cfg.flying {
        let rotation = camera_world
            .map(math::mat_to_quat)
            .unwrap_or([0., 0., 0., 1.]);
        (
            math::quat_rotate_vec3(rotation, [1., 0., 0.]),
            math::quat_rotate_vec3(rotation, [0., 0., 1.]),
        )
    } else {
        camera_world
            .and_then(horizontal_camera_basis)
            .unwrap_or(([1., 0., 0.], [0., 0., 1.]))
    };
    let mut move_world =
        std::array::from_fn(|i| right[i] * dx + back[i] * dz + if i == 1 { vertical } else { 0. });
    let length = math::vec3_len(move_world);
    if length > 1. {
        move_world = move_world.map(|v| v / length);
    }

    let speed = cfg.speed * if cfg.flying { 2.0 } else { 1.0 };
    let delta_world = move_world.map(|v| v * speed * dt_sec);

    // The locomotion target stores a local translation. Convert the world-space
    // direction back through its parent so rotated parent rigs don't skew it.
    let parent_world = world
        .parent_of(target_tcid)
        .and_then(|parent| TransformSystem::world_model(world, parent));
    let delta_local = parent_world
        .map(|parent_world| {
            math::quat_rotate_vec3(
                math::quat_conjugate(math::mat_to_quat(parent_world)),
                delta_world,
            )
        })
        .unwrap_or(delta_world);
    // Turn about the headset rather than the rig origin, so a player standing
    // away from the play-space center spins in place instead of orbiting.
    let turn_frame = (turn != 0.).then(|| {
        let pivot = camera_world.map(|m| math::vec3_add([m[3][0], m[3][1], m[3][2]], delta_world));
        let Some(inverse) = parent_world.and_then(math::mat4_inverse) else {
            return (pivot, [0., 1., 0.]);
        };
        let to_local = |v: [f32; 3], w: f32| {
            let local = math::mat4_mul_vec4(inverse, [v[0], v[1], v[2], w]);
            [local[0], local[1], local[2]]
        };
        (pivot.map(|p| to_local(p, 1.)), to_local([0., 1., 0.], 0.))
    });
    let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(target_tcid) else {
        return false;
    };
    for axis in 0..3 {
        t.transform.translation[axis] += delta_local[axis];
    }
    if let Some((pivot, up)) = turn_frame {
        // Pushing right turns clockwise seen from above: negative yaw.
        let yaw = math::quat_from_axis_angle(up, -turn * cfg.turn_speed * dt_sec);
        if let Some(pivot) = pivot {
            let offset = math::vec3_sub(t.transform.translation, pivot);
            t.transform.translation = math::vec3_add(pivot, math::quat_rotate_vec3(yaw, offset));
        }
        t.transform.rotation = math::quat_normalize(math::quat_mul(yaw, t.transform.rotation));
    }
    t.transform.recompute_model();

    let transform = t.transform;
    emit.push_intent_now(
        target_tcid,
        IntentValue::UpdateTransform {
            component_id: target_tcid,
            translation: transform.translation,
            rotation_quat_xyzw: transform.rotation,
            scale: transform.scale,
        },
    );
    true
}

fn radial_deadzone(v: [f32; 2], deadzone: f32) -> [f32; 2] {
    let len = (v[0] * v[0] + v[1] * v[1]).sqrt();
    if len <= deadzone || len <= f32::EPSILON {
        return [0.; 2];
    }
    let scaled = ((len - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0);
    [v[0] / len * scaled, v[1] / len * scaled]
}

fn axis_deadzone(v: f32, deadzone: f32) -> f32 {
    if v.abs() <= deadzone {
        return 0.;
    }
    v.signum() * ((v.abs() - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0)
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

#[cfg(test)]
#[path = "input_xr_gamepad_tests.rs"]
mod tests;
