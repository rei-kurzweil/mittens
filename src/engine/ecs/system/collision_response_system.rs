use crate::engine::ecs::component::{
    CollisionComponent, CollisionMode, CollisionResponseComponent, CollisionResponseMode,
    CollisionShape, CollisionShapeComponent, QueryRootMode, RenderableComponent,
    TransformComponent, resolve_component_ref,
};
use crate::engine::ecs::system::{CollisionSystem, TransformSystem, collision_geometry};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;
use crate::utils::math;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct CollisionResponseSystem {
    responders: Vec<ComponentId>,
}

impl CollisionResponseSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_collision_response(&mut self, world: &mut World, component: ComponentId) {
        if !self.responders.iter().any(|c| *c == component) {
            self.responders.push(component);
        }

        // Cache gravity coefficient from the nearest enabled GravityComponent ancestor.
        let mut cur = component;
        let mut gravity_coef = 0.0f32;
        while let Some(parent) = world.parent_of(cur) {
            if let Some(g) = world
                .get_component_by_id_as::<crate::engine::ecs::component::GravityComponent>(parent)
            {
                if g.enabled {
                    gravity_coef = g.coefficient;
                    break;
                }
            }
            cur = parent;
        }

        if let Some(r) = world.get_component_by_id_as_mut::<CollisionResponseComponent>(component) {
            r.gravity_coefficient = gravity_coef;
        }
    }

    pub fn remove_collision_response(&mut self, component: ComponentId) {
        self.responders.retain(|c| *c != component);
    }

    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        dt_sec: f32,
        emit: &mut dyn SignalEmitter,
        collision: &CollisionSystem,
    ) {
        if self.responders.is_empty() {
            return;
        }

        let pairs = collision.active_pairs_with_delta_snapshot();

        // Build an adjacency index once per tick so we don't scan all pairs per responder.
        // Key: collider cid, Value: other colliders overlapping it.
        // Note: pairs can be empty; Push mode still needs to integrate velocity.
        let mut overlaps_by_collider: HashMap<ComponentId, Vec<(ComponentId, [f32; 3])>> =
            HashMap::new();
        if !pairs.is_empty() {
            for (a, b, delta_ab) in pairs.iter().copied() {
                // Store the vector from other -> self (so it matches `self_pos - other_pos`).
                overlaps_by_collider
                    .entry(a)
                    .or_default()
                    .push((b, [-delta_ab[0], -delta_ab[1], -delta_ab[2]]));
                overlaps_by_collider
                    .entry(b)
                    .or_default()
                    .push((a, delta_ab));
            }
        }

        let responder_ids: Vec<ComponentId> = self.responders.iter().copied().collect();
        let mut pending_updates: Vec<(ComponentId, ComponentId, [f32; 3], [f32; 3])> = Vec::new();

        for response_cid in responder_ids {
            let Some(response) =
                world.get_component_by_id_as::<CollisionResponseComponent>(response_cid)
            else {
                continue;
            };
            if !response.enabled {
                continue;
            }
            let movement_target_source = response.movement_target_source.clone();
            let runtime_movement_target = response.movement_target_id;
            let movement_target_required = response.movement_target_required;

            // Required topology:
            //   TransformComponent { CollisionComponent { CollisionResponseComponent { ... } } }
            let Some(collider_cid) = world.parent_of(response_cid) else {
                continue;
            };
            if world
                .get_component_by_id_as::<CollisionComponent>(collider_cid)
                .is_none()
            {
                continue;
            }

            let Some(transform_cid) = world.parent_of(collider_cid) else {
                continue;
            };
            if world
                .get_component_by_id_as::<TransformComponent>(transform_cid)
                .is_none()
            {
                continue;
            }

            let movement_target_cid = if let Some(source) = movement_target_source.as_ref() {
                let Some(target) = resolve_component_ref(
                    world,
                    source,
                    Some(response_cid),
                    QueryRootMode::SelfSubtree,
                ) else {
                    // An authored target is authoritative: do not integrate velocity while
                    // it is temporarily unavailable.
                    continue;
                };
                target
            } else if let Some(target) = runtime_movement_target {
                target
            } else if movement_target_required {
                continue;
            } else {
                transform_cid
            };
            if world
                .get_component_by_id_as::<TransformComponent>(movement_target_cid)
                .is_none()
            {
                continue;
            }

            // Only treat kinematic/rigged colliders as responders.
            let Some(collider) = world.get_component_by_id_as::<CollisionComponent>(collider_cid)
            else {
                continue;
            };
            if collider.mode == CollisionMode::Static {
                continue;
            }

            let gravity_coef = response.gravity_coefficient;

            let overlaps: Vec<(ComponentId, [f32; 3])> = overlaps_by_collider
                .get(&collider_cid)
                .cloned()
                .unwrap_or_default();

            // Split overlaps into static and non-static colliders.
            let mut statics: Vec<ComponentId> = Vec::new();
            let mut non_statics: Vec<(ComponentId, [f32; 3])> = Vec::new();
            for (other, delta_other_to_self) in overlaps {
                let Some(c) = world.get_component_by_id_as::<CollisionComponent>(other) else {
                    continue;
                };
                if c.mode == CollisionMode::Static {
                    statics.push(other);
                } else {
                    non_statics.push((other, delta_other_to_self));
                }
            }

            // Slide mode only does static separation; no overlaps means no work.
            if response.mode == CollisionResponseMode::Slide && statics.is_empty() {
                continue;
            }

            let mut moved = false;
            let mut desired_world_pos = match TransformSystem::world_position(world, transform_cid)
            {
                Some(p) => p,
                None => continue,
            };
            let base_world_pos = desired_world_pos;

            // Mode-dependent motion intent.
            let mut velocity = response.velocity;

            // Gravity is applied for any mode when GravityComponent is present.
            if dt_sec > 0.0 && gravity_coef != 0.0 {
                const GRAVITY_MPS2: f32 = -9.81;
                velocity[1] += GRAVITY_MPS2 * gravity_coef * dt_sec;
            }

            if response.mode == CollisionResponseMode::Push {
                // Apply friction-like damping.
                if dt_sec > 0.0 && response.friction > 0.0 {
                    let k = (1.0 - response.friction * dt_sec).clamp(0.0, 1.0);
                    velocity[0] *= k;
                    velocity[1] *= k;
                    velocity[2] *= k;
                }

                // Overlaps with non-static colliders produce acceleration away from them.
                if !non_statics.is_empty() && dt_sec > 0.0 && response.push_strength != 0.0 {
                    let mut sum = [0.0f32; 3];
                    let mut n = 0.0f32;
                    let self_motion = [
                        desired_world_pos[0] - base_world_pos[0],
                        desired_world_pos[1] - base_world_pos[1],
                        desired_world_pos[2] - base_world_pos[2],
                    ];

                    for &(_other_cid, delta_other_to_self_at_base) in non_statics.iter() {
                        // Adjust delta to reflect our integrated desired_world_pos.
                        let adjusted = [
                            delta_other_to_self_at_base[0] + self_motion[0],
                            delta_other_to_self_at_base[1] + self_motion[1],
                            delta_other_to_self_at_base[2] + self_motion[2],
                        ];

                        // Delta from other -> self (desired).
                        sum[0] += adjusted[0];
                        sum[1] += adjusted[1];
                        sum[2] += adjusted[2];
                        n += 1.0;
                    }

                    if n > 0.0 {
                        let avg = [sum[0] / n, sum[1] / n, sum[2] / n];
                        velocity[0] += avg[0] * response.push_strength * dt_sec;
                        velocity[1] += avg[1] * response.push_strength * dt_sec;
                        velocity[2] += avg[2] * response.push_strength * dt_sec;
                    }
                }

                // Clamp speed.
                if response.max_speed > 0.0 {
                    let speed = (velocity[0] * velocity[0]
                        + velocity[1] * velocity[1]
                        + velocity[2] * velocity[2])
                        .sqrt();
                    if speed > response.max_speed {
                        let s = response.max_speed / speed;
                        velocity[0] *= s;
                        velocity[1] *= s;
                        velocity[2] *= s;
                    }
                }

                // Integrate.
                desired_world_pos[0] += velocity[0] * dt_sec;
                desired_world_pos[1] += velocity[1] * dt_sec;
                desired_world_pos[2] += velocity[2] * dt_sec;
                moved = moved || velocity != [0.0, 0.0, 0.0];
            }

            // Slide mode typically does no integration; gravity-enabled colliders still need
            // vertical integration so they can fall onto static geometry.
            if response.mode == CollisionResponseMode::Slide && dt_sec > 0.0 {
                if velocity[1] != 0.0 {
                    desired_world_pos[1] += velocity[1] * dt_sec;
                    moved = true;
                }
            }

            let a_shape = resolve_shape(world, collider_cid)
                .unwrap_or_else(|| crate::engine::ecs::component::CollisionShape::CUBE());

            // Static separation (applies to both Slide and Push modes).
            for _ in 0..response.max_iterations {
                let mut any_overlap = false;

                for &static_cid in statics.iter() {
                    let Some(static_parent) = world.parent_of(static_cid) else {
                        continue;
                    };
                    if world
                        .get_component_by_id_as::<TransformComponent>(static_parent)
                        .is_none()
                    {
                        continue;
                    }

                    let b_world_pos = match TransformSystem::world_position(world, static_parent) {
                        Some(p) => p,
                        None => continue,
                    };
                    let b_shape = resolve_shape(world, static_cid)
                        .unwrap_or_else(|| crate::engine::ecs::component::CollisionShape::CUBE());

                    let Some(push) = collision_geometry::minimum_translation(
                        desired_world_pos,
                        a_shape,
                        b_world_pos,
                        b_shape,
                        response.push_out_epsilon,
                    ) else {
                        continue;
                    };

                    desired_world_pos[0] += push[0];
                    desired_world_pos[1] += push[1];
                    desired_world_pos[2] += push[2];

                    let len = (push[0] * push[0] + push[1] * push[1] + push[2] * push[2]).sqrt();
                    let normal = if len > f32::EPSILON {
                        [push[0] / len, push[1] / len, push[2] / len]
                    } else {
                        [0.0, 1.0, 0.0]
                    };
                    let vertical_contact = normal[1].abs() >= normal[0].abs().max(normal[2].abs());

                    if vertical_contact {
                        // Floor/ceiling contact: dampen vertical velocity only.
                        if dt_sec > 0.0 && response.friction_y > 0.0 {
                            let k = (1.0 - response.friction_y * dt_sec).clamp(0.0, 1.0);
                            velocity[1] *= k;
                        }
                    } else if response.mode == CollisionResponseMode::Push {
                        // Push-mode "bounce" on horizontal static contacts.
                        // Without this, a body with outward velocity will just keep trying to
                        // move into the wall and get corrected every tick (looks like sticking).
                        let into = velocity[0] * normal[0]
                            + velocity[1] * normal[1]
                            + velocity[2] * normal[2];
                        if into < 0.0 {
                            const RESTITUTION: f32 = 0.85;
                            velocity[0] -= (1.0 + RESTITUTION) * into * normal[0];
                            velocity[1] -= (1.0 + RESTITUTION) * into * normal[1];
                            velocity[2] -= (1.0 + RESTITUTION) * into * normal[2];
                        }
                    }
                    any_overlap = true;
                }

                if !any_overlap {
                    break;
                }
                moved = true;
            }

            if !moved {
                continue;
            }

            let displacement = [
                desired_world_pos[0] - base_world_pos[0],
                desired_world_pos[1] - base_world_pos[1],
                desired_world_pos[2] - base_world_pos[2],
            ];
            let Some(target_world_pos) =
                TransformSystem::world_position(world, movement_target_cid)
            else {
                continue;
            };
            let target_desired_world = [
                target_world_pos[0] + displacement[0],
                target_world_pos[1] + displacement[1],
                target_world_pos[2] + displacement[2],
            ];
            let new_local_translation =
                world_to_local_translation(world, movement_target_cid, target_desired_world);

            pending_updates.push((
                response_cid,
                movement_target_cid,
                new_local_translation,
                velocity,
            ));
        }

        for (response_cid, transform_cid, new_local_translation, new_velocity) in pending_updates {
            if let Some(r) =
                world.get_component_by_id_as_mut::<CollisionResponseComponent>(response_cid)
            {
                r.velocity = new_velocity;
            }
            if let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(transform_cid) {
                t.transform.translation = new_local_translation;
                t.transform.recompute_model();
                let transform = t.transform;
                emit.push_intent_now(
                    transform_cid,
                    IntentValue::UpdateTransform {
                        component_ids: vec![transform_cid],
                        translation: transform.translation,
                        rotation_quat_xyzw: transform.rotation,
                        scale: transform.scale,
                    },
                );
            }
        }
    }
}

fn resolve_shape(world: &World, collision_cid: ComponentId) -> Option<CollisionShape> {
    for child in world.children_of(collision_cid) {
        if let Some(s) = world.get_component_by_id_as::<CollisionShapeComponent>(*child) {
            return Some(s.shape);
        }
    }

    let parent = world.parent_of(collision_cid)?;
    for sib in world.children_of(parent) {
        if *sib == collision_cid {
            continue;
        }
        let Some(r) = world.get_component_by_id_as::<RenderableComponent>(*sib) else {
            continue;
        };

        if r.renderable.base_mesh == crate::engine::graphics::primitives::CpuMeshHandle::CUBE {
            return Some(CollisionShape::CUBE());
        }
        if r.renderable.base_mesh == crate::engine::graphics::primitives::CpuMeshHandle::SPHERE {
            return Some(CollisionShape::SPHERE());
        }
    }

    None
}

fn mat4_mul_vec4(m: [[f32; 4]; 4], v: [f32; 4]) -> [f32; 4] {
    [
        m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2] + m[3][0] * v[3],
        m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2] + m[3][1] * v[3],
        m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2] + m[3][2] * v[3],
        m[0][3] * v[0] + m[1][3] * v[1] + m[2][3] * v[2] + m[3][3] * v[3],
    ]
}

fn world_to_local_translation(
    world: &World,
    transform_cid: ComponentId,
    desired_world: [f32; 3],
) -> [f32; 3] {
    let mut cur = transform_cid;
    while let Some(parent) = world.parent_of(cur) {
        if let Some(t) = world.get_component_by_id_as::<TransformComponent>(parent) {
            if let Some(inv) = math::mat4_inverse(t.transform.matrix_world) {
                let p_local = mat4_mul_vec4(
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
