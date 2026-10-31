//! XR pointer grips translate the tracking origin to keep the active hand in place.
use crate::engine::ecs::component::{
    ClimbableComponent, ControllerXRComponent, InputXRComponent, PointerComponent,
    TransformComponent, VelocityComponent,
};
use crate::engine::ecs::system::grabbable_system::resolve_marker_target;
use crate::engine::ecs::system::input_xr_gamepad_system::xr_locomotion_target_transform;
use crate::engine::ecs::system::{TransformSystem, VelocitySystem};
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, RxWorld, SignalEmitter, SignalKind, World,
};
use crate::utils::math::{mat4_inverse, mat4_mul_vec4};
use std::sync::{Arc, Mutex};

pub(crate) fn climbable_owner_for_hit(
    world: &World,
    renderable: ComponentId,
) -> Option<ComponentId> {
    resolve_marker_target::<ClimbableComponent>(world, renderable, |c| (c.enabled, false))
}

#[derive(Debug, Clone, Copy)]
enum Command {
    Start {
        pointer: ComponentId,
        renderable: ComponentId,
    },
    End(ComponentId),
}
#[derive(Debug)]
struct Grip {
    pointer: ComponentId,
    renderable: ComponentId,
    target: ComponentId,
    anchor: Option<[f32; 3]>,
}
#[derive(Debug, Default)]
pub(crate) struct ClimbingSystem {
    installed: bool,
    commands: Arc<Mutex<Vec<Command>>>,
    grips: Vec<Grip>,
    pub moved: bool,
}

impl ClimbingSystem {
    pub fn install_handlers(&mut self, rx: &mut RxWorld) {
        if self.installed {
            return;
        }
        let starts = self.commands.clone();
        rx.add_global_handler_closure(SignalKind::GrabStart, move |_world, _emit, env| {
            if let Some(EventSignal::GrabStart {
                pointer,
                renderable,
                ..
            }) = env.event.as_ref()
            {
                if let Ok(mut commands) = starts.lock() {
                    commands.push(Command::Start {
                        pointer: *pointer,
                        renderable: *renderable,
                    });
                }
            }
        });
        let ends = self.commands.clone();
        rx.add_global_handler_closure(SignalKind::GrabEnd, move |_world, _emit, env| {
            if let Some(EventSignal::GrabEnd { pointer, .. }) = env.event.as_ref() {
                if let Ok(mut commands) = ends.lock() {
                    commands.push(Command::End(*pointer));
                }
            }
        });
        self.installed = true;
    }

    /// Stop accumulated momentum while held; gravity resumes normally on release.
    pub fn prepare_velocity(&self, world: &mut World) {
        for grip in &self.grips {
            let Some(parent) = world.parent_of(grip.target) else {
                continue;
            };
            if VelocitySystem::driven_transform(world, parent).ok() == Some(grip.target) {
                if let Some(velocity) =
                    world.get_component_by_id_as_mut::<VelocityComponent>(parent)
                {
                    velocity.zero_linear();
                }
            }
        }
    }

    pub fn tick(&mut self, world: &mut World, emit: &mut dyn SignalEmitter) {
        self.moved = false;
        let commands = self
            .commands
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default();
        for command in commands {
            match command {
                Command::Start {
                    pointer,
                    renderable,
                } => {
                    if self.grips.iter().any(|g| g.pointer == pointer)
                        || climbable_owner_for_hit(world, renderable).is_none()
                    {
                        continue;
                    }
                    let mut current = world.parent_of(pointer);
                    let mut controller = false;
                    let mut input = None;
                    while let Some(id) = current {
                        controller |= world
                            .get_component_by_id_as::<ControllerXRComponent>(id)
                            .is_some();
                        if world
                            .get_component_by_id_as::<InputXRComponent>(id)
                            .is_some()
                        {
                            input = Some(id);
                            break;
                        }
                        current = world.parent_of(id);
                    }
                    let Some(target) = input
                        .filter(|_| controller)
                        .and_then(|id| xr_locomotion_target_transform(world, id))
                    else {
                        continue;
                    };
                    let Some(anchor) = TransformSystem::world_position(world, pointer) else {
                        continue;
                    };
                    self.grips.push(Grip {
                        pointer,
                        renderable,
                        target,
                        anchor: Some(anchor),
                    });
                }
                Command::End(pointer) => {
                    if let Some(index) = self.grips.iter().position(|g| g.pointer == pointer) {
                        let target = self.grips[index].target;
                        let was_primary =
                            !self.grips[index + 1..].iter().any(|g| g.target == target);
                        self.grips.remove(index);
                        // The next hand takes over at its current position without a jump.
                        if was_primary {
                            if let Some(grip) =
                                self.grips.iter_mut().rev().find(|g| g.target == target)
                            {
                                grip.anchor = None;
                            }
                        }
                    }
                }
            }
        }
        let mut i = 0;
        while i < self.grips.len() {
            let grip = &self.grips[i];
            let valid = world
                .get_component_by_id_as::<PointerComponent>(grip.pointer)
                .is_some()
                && world
                    .get_component_by_id_as::<TransformComponent>(grip.target)
                    .is_some()
                && climbable_owner_for_hit(world, grip.renderable).is_some();
            if valid {
                i += 1;
            } else {
                let target = self.grips[i].target;
                let was_primary = !self.grips[i + 1..].iter().any(|g| g.target == target);
                self.grips.remove(i);
                if was_primary {
                    if let Some(grip) = self.grips.iter_mut().rev().find(|g| g.target == target) {
                        grip.anchor = None;
                    }
                }
            }
        }
        // Most recently acquired hand controls each rig; independent rigs can climb simultaneously.
        for i in 0..self.grips.len() {
            if self.grips[i + 1..]
                .iter()
                .any(|g| g.target == self.grips[i].target)
            {
                continue;
            }
            let grip = &mut self.grips[i];
            let Some(position) = TransformSystem::world_position(world, grip.pointer) else {
                continue;
            };
            let Some(anchor) = grip.anchor else {
                grip.anchor = Some(position);
                continue;
            };
            let delta = [
                anchor[0] - position[0],
                anchor[1] - position[1],
                anchor[2] - position[2],
            ];
            let local = match world
                .parent_of(grip.target)
                .and_then(|p| TransformSystem::world_model(world, p))
            {
                Some(parent) => {
                    let Some(inverse) = mat4_inverse(parent) else {
                        continue;
                    };
                    let v = mat4_mul_vec4(inverse, [delta[0], delta[1], delta[2], 0.0]);
                    [v[0], v[1], v[2]]
                }
                None => delta,
            };
            if !local.iter().all(|v| v.is_finite()) || local == [0.0; 3] {
                continue;
            }
            let Some(transform) =
                world.get_component_by_id_as_mut::<TransformComponent>(grip.target)
            else {
                continue;
            };
            for (axis, d) in local.into_iter().enumerate() {
                transform.transform.translation[axis] += d;
            }
            transform.transform.recompute_model();
            self.moved = true;
            let t = transform.transform;
            emit.push_intent_now(
                grip.target,
                IntentValue::UpdateTransform {
                    component_id: grip.target,
                    translation: t.translation,
                    rotation_quat_xyzw: t.rotation,
                    scale: t.scale,
                },
            );
        }
    }
}

#[cfg(test)]
#[path = "climbing_tests.rs"]
mod tests;
