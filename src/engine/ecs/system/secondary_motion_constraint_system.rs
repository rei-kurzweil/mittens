use crate::engine::ecs::component::{
    ReturnToRestWhenStillComponent, SpringBoneComponent, TransformComponent,
};
use crate::engine::ecs::system::SecondaryMotionSystem;
use crate::engine::ecs::{ComponentId, World};
use crate::utils::math::{mat_to_quat, mat4_inverse, mat4_mul, vec3_len, vec3_sub};
use std::collections::HashMap;

/// Evaluates authored secondary-motion constraints before spring integration.
/// The spring system remains the only writer of imported joint transforms.
#[derive(Debug, Default)]
pub struct SecondaryMotionConstraintSystem {
    constraints: HashMap<ComponentId, ConstraintState>,
    directives: HashMap<ComponentId, f32>,
    debug_frames: u64,
}

#[derive(Debug)]
struct ConstraintState {
    chain: ComponentId,
    anchor: Option<ComponentId>,
    gltf: Option<ComponentId>,
    basis: Option<ComponentId>,
    previous: Option<([f32; 3], [f32; 4])>,
    smoothed_speed: f32,
    still_seconds: f32,
    weight: f32,
}

impl ConstraintState {
    fn new(chain: ComponentId) -> Self {
        Self {
            chain,
            anchor: None,
            gltf: None,
            basis: None,
            previous: None,
            smoothed_speed: 0.0,
            still_seconds: 0.0,
            weight: 0.0,
        }
    }

    fn reset_binding(&mut self, world: &World, anchor: ComponentId, gltf: ComponentId) {
        self.anchor = Some(anchor);
        self.gltf = Some(gltf);
        self.basis = parent_transform_above(world, gltf);
        self.previous = None;
        self.smoothed_speed = 0.0;
        self.still_seconds = 0.0;
        self.weight = 0.0;
    }

    fn advance(
        &mut self,
        config: &ReturnToRestWhenStillComponent,
        position: [f32; 3],
        rotation: [f32; 4],
        dt: f32,
    ) {
        if let Some((last_position, last_rotation)) = self.previous {
            let linear = vec3_len(vec3_sub(position, last_position)) / dt;
            let dot = rotation
                .iter()
                .zip(last_rotation)
                .map(|(a, b)| a * b)
                .sum::<f32>()
                .abs()
                .clamp(0.0, 1.0);
            let angular = 2.0 * dot.acos() / dt;
            let speed = linear + angular * 0.1;
            let alpha = 1.0 - (-dt / 0.1).exp();
            self.smoothed_speed += (speed - self.smoothed_speed) * alpha;
            if self.smoothed_speed <= config.motion_threshold {
                self.still_seconds += dt;
            } else if self.smoothed_speed > config.motion_threshold * 1.5 {
                self.still_seconds = 0.0;
            }
            let target = if self.still_seconds >= config.still_for {
                1.0
            } else {
                0.0
            };
            let duration = if target > self.weight { 0.25 } else { 0.12 };
            self.weight += (target - self.weight).clamp(-dt / duration, dt / duration);
        }
        self.previous = Some((position, rotation));
    }
}

impl SecondaryMotionConstraintSystem {
    pub fn register(&mut self, world: &World, id: ComponentId) {
        if world
            .get_component_by_id_as::<ReturnToRestWhenStillComponent>(id)
            .is_none()
        {
            return;
        }
        let Some(chain) = world.parent_of(id).filter(|parent| {
            world
                .get_component_by_id_as::<SpringBoneComponent>(*parent)
                .is_some()
        }) else {
            eprintln!("[SecondaryMotionConstraint] {id:?} must be a child of SpringBone");
            return;
        };
        if self
            .constraints
            .iter()
            .any(|(&other, state)| other != id && state.chain == chain)
        {
            eprintln!(
                "[SecondaryMotionConstraint] chain {chain:?} has more than one pose constraint"
            );
            return;
        }
        self.constraints
            .entry(id)
            .and_modify(|state| {
                if state.chain != chain {
                    *state = ConstraintState::new(chain);
                }
            })
            .or_insert_with(|| ConstraintState::new(chain));
    }

    pub fn topology_changed(&mut self, world: &World, id: ComponentId) {
        if self.constraints.remove(&id).is_some() {
            self.register(world, id);
        } else if world
            .get_component_by_id_as::<ReturnToRestWhenStillComponent>(id)
            .is_some()
        {
            self.register(world, id);
        }
        for state in self.constraints.values_mut() {
            if state.chain == id
                || state.anchor == Some(id)
                || state.gltf == Some(id)
                || state.basis == Some(id)
            {
                state.anchor = None;
                state.previous = None;
                state.still_seconds = 0.0;
                state.weight = 0.0;
            }
        }
    }

    pub fn component_removed(&mut self, id: ComponentId) {
        self.constraints
            .retain(|&constraint, state| constraint != id && state.chain != id);
        for state in self.constraints.values_mut() {
            if state.anchor == Some(id) || state.gltf == Some(id) || state.basis == Some(id) {
                state.anchor = None;
                state.previous = None;
                state.still_seconds = 0.0;
                state.weight = 0.0;
            }
        }
        self.directives.remove(&id);
    }

    pub fn tick(&mut self, world: &World, spring: &SecondaryMotionSystem, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 || dt > 0.25 {
            return;
        }
        self.debug_frames = self.debug_frames.wrapping_add(1);
        let debug = self.debug_frames % 120 == 0
            && std::env::var_os("CAT_DEBUG_SECONDARY_MOTION").is_some();
        self.directives.clear();
        for (&id, state) in &mut self.constraints {
            let Some(config) = world.get_component_by_id_as::<ReturnToRestWhenStillComponent>(id)
            else {
                continue;
            };
            let Some((anchor, gltf)) = spring.chain_anchor(state.chain) else {
                state.previous = None;
                continue;
            };
            if state.anchor != Some(anchor) || state.gltf != Some(gltf) {
                state.reset_binding(world, anchor, gltf);
            }
            let Some((position, rotation)) = sampled_pose(world, anchor, state.basis) else {
                continue;
            };
            state.advance(config, position, rotation, dt);
            self.directives.insert(state.chain, state.weight);
            if debug {
                eprintln!(
                    "[SecondaryMotionConstraint][debug] chain={:?} speed={:.4} threshold={:.4} still_for={:.2} elapsed={:.2} rest_weight={:.2}",
                    state.chain,
                    state.smoothed_speed,
                    config.motion_threshold,
                    config.still_for,
                    state.still_seconds,
                    state.weight,
                );
            }
        }
    }

    pub fn directives(&self) -> &HashMap<ComponentId, f32> {
        &self.directives
    }
}

fn parent_transform_above(world: &World, gltf: ComponentId) -> Option<ComponentId> {
    let mut ancestor = gltf;
    for _ in 0..64 {
        ancestor = world.parent_of(ancestor)?;
        if world
            .get_component_by_id_as::<TransformComponent>(ancestor)
            .is_some()
        {
            return Some(ancestor);
        }
    }
    None
}

fn sampled_pose(
    world: &World,
    anchor: ComponentId,
    basis: Option<ComponentId>,
) -> Option<([f32; 3], [f32; 4])> {
    let anchor_world = world
        .get_component_by_id_as::<TransformComponent>(anchor)?
        .transform
        .matrix_world;
    let basis_world = basis
        .and_then(|id| world.get_component_by_id_as::<TransformComponent>(id))
        .map(|t| t.transform.matrix_world);
    let relative = if let Some(matrix) = basis_world {
        mat4_mul(mat4_inverse(matrix)?, anchor_world)
    } else {
        anchor_world
    };
    Some((
        [relative[3][0], relative[3][1], relative[3][2]],
        mat_to_quat(relative),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stillness_settles_and_motion_wakes() {
        let mut world = World::default();
        let chain = world.add_component(TransformComponent::new());
        let mut state = ConstraintState::new(chain);
        let config = ReturnToRestWhenStillComponent::new()
            .motion_threshold(0.02)
            .still_for(0.4);
        let identity = [0.0, 0.0, 0.0, 1.0];
        for _ in 0..60 {
            state.advance(&config, [0.0; 3], identity, 1.0 / 60.0);
        }
        assert_eq!(state.weight, 1.0);
        state.advance(&config, [0.5, 0.0, 0.0], identity, 1.0 / 60.0);
        assert!(state.weight < 1.0);
        assert_eq!(state.still_seconds, 0.0);
    }
}
