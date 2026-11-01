//! Opt-in XR flight mode. Physics remains responsible for solid contacts.
use super::{VelocitySystem, XrGamepadState};
use crate::engine::ecs::component::{InputXRGamepadComponent, VelocityComponent};
use crate::engine::ecs::{ComponentId, World};
use std::collections::HashMap;

#[derive(Debug, Default)]
struct FlightState {
    down: bool,
    last_tap: Option<f64>,
    velocity: Option<ComponentId>,
}
#[derive(Debug, Default)]
pub(super) struct XrFlying {
    clock: f64,
    states: HashMap<ComponentId, FlightState>,
}
impl XrFlying {
    pub fn advance(&mut self, dt: f32) {
        if dt.is_finite() && dt > 0. {
            self.clock += dt as f64;
        }
    }
    pub fn stop(&mut self, world: &mut World, id: ComponentId) {
        if let Some(state) = self.states.remove(&id) {
            if let Some(velocity) = state
                .velocity
                .and_then(|v| world.get_component_by_id_as_mut::<VelocityComponent>(v))
            {
                if velocity.flying {
                    velocity.zero_linear();
                }
                velocity.flying = false;
            }
        }
        if let Some(input) = world.get_component_by_id_as_mut::<InputXRGamepadComponent>(id) {
            input.flying = false;
        }
    }
    pub fn suspend(&mut self, world: &mut World) {
        self.states.retain(|&id, state| {
            let valid = world
                .get_component_by_id_as::<InputXRGamepadComponent>(id)
                .is_some_and(|c| c.enabled && c.locomotion && c.enable_flying);
            state.down = false;
            state.last_tap = None;
            if let Some(v) = state
                .velocity
                .and_then(|v| world.get_component_by_id_as_mut::<VelocityComponent>(v))
            {
                if v.flying {
                    v.zero_linear();
                }
                if !valid {
                    v.flying = false;
                }
            }
            if !valid {
                if let Some(input) = world.get_component_by_id_as_mut::<InputXRGamepadComponent>(id)
                {
                    input.flying = false;
                }
            }
            // Keep a valid flying avatar hovering during temporary input loss.
            valid
        });
    }
    pub fn update(
        &mut self,
        world: &mut World,
        id: ComponentId,
        target: ComponentId,
        xr: &XrGamepadState,
    ) -> bool {
        let Some(config) = world.get_component_by_id_as::<InputXRGamepadComponent>(id) else {
            self.stop(world, id);
            return false;
        };
        if !config.enabled || !config.locomotion || !config.enable_flying {
            self.stop(world, id);
            return false;
        }
        let mut flying = config.flying;
        let mut ancestor = world.parent_of(target);
        let mut velocity = None;
        while let Some(node) = ancestor {
            if world
                .get_component_by_id_as::<VelocityComponent>(node)
                .is_some()
            {
                if VelocitySystem::driven_transform(world, node).ok() == Some(target) {
                    velocity = Some(node);
                }
                break;
            }
            ancestor = world.parent_of(node);
        }
        let state = self.states.entry(id).or_default();
        if state.velocity != velocity {
            if let Some(old) = state
                .velocity
                .and_then(|old| world.get_component_by_id_as_mut::<VelocityComponent>(old))
            {
                old.flying = false;
            }
            state.velocity = velocity;
            state.last_tap = None;
        }
        let down = xr.hands[1].button_b.is_some_and(|b| b.0);
        if down && !state.down {
            if state
                .last_tap
                .take()
                .is_some_and(|t| self.clock - t < 0.250)
            {
                flying = !flying;
                if let Some(v) =
                    velocity.and_then(|v| world.get_component_by_id_as_mut::<VelocityComponent>(v))
                {
                    v.zero_linear();
                    v.grounded = false;
                }
            } else {
                state.last_tap = Some(self.clock);
                if !flying {
                    if let Some(velocity_id) = velocity {
                        if world
                            .get_component_by_id_as::<VelocityComponent>(velocity_id)
                            .is_some_and(|v| v.grounded && v.enabled)
                        {
                            let rotation = VelocitySystem::parent_rotation(world, velocity_id)
                                .unwrap_or([0., 0., 0., 1.]);
                            let jump = crate::utils::math::quat_rotate_vec3(
                                crate::utils::math::quat_conjugate(rotation),
                                [0., 4.5, 0.],
                            );
                            let v = world
                                .get_component_by_id_as_mut::<VelocityComponent>(velocity_id)
                                .unwrap();
                            v.linear_local_mps = jump;
                            v.grounded = false;
                        }
                    }
                }
            }
        }
        state.down = down;
        world
            .get_component_by_id_as_mut::<InputXRGamepadComponent>(id)
            .unwrap()
            .flying = flying;
        if let Some(v) =
            velocity.and_then(|v| world.get_component_by_id_as_mut::<VelocityComponent>(v))
        {
            v.flying = flying;
            if flying {
                v.grounded = false;
            }
        }
        flying
    }
}

/// Flying is an explicit ungrounded mode, including while touching a floor.
pub(crate) fn is_flying(world: &World, start: ComponentId) -> bool {
    let mut node = Some(start);
    while let Some(id) = node {
        if world
            .get_component_by_id_as::<VelocityComponent>(id)
            .is_some_and(|v| v.flying)
        {
            return true;
        }
        if world.children_of(id).iter().any(|&child| {
            world
                .get_component_by_id_as::<InputXRGamepadComponent>(child)
                .is_some_and(|c| c.enabled && c.enable_flying && c.flying)
        }) {
            return true;
        }
        node = world.parent_of(id);
    }
    false
}
