use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{
    CollisionComponent, CollisionShape, CollisionShapeComponent, PhysicsBodyComponent,
    PhysicsBodyMode, RenderableComponent, TransformComponent,
};
use crate::engine::ecs::system::{CollisionSystem, TransformSystem};
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;
use crate::utils::math;

#[derive(Debug, Default)]
pub struct PhysicsSystem {
    bodies: Vec<ComponentId>,
}

impl PhysicsSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_physics_body(&mut self, component: ComponentId) {
        if !self.bodies.iter().any(|c| *c == component) {
            self.bodies.push(component);
        }
    }

    pub fn remove_physics_body(&mut self, component: ComponentId) {
        self.bodies.retain(|c| *c != component);
    }

    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
        queue: &mut crate::engine::ecs::CommandQueue,
        collision: &CollisionSystem,
    ) {
        let pairs = collision.active_pairs_snapshot();
        if pairs.is_empty() {
            return;
        }

        // For each registered PhysicsBodyComponent, try to resolve overlaps for its associated collider.
        for &physics_cid in self.bodies.clone().iter() {
            let Some(body) = world.get_component_by_id_as::<PhysicsBodyComponent>(physics_cid) else {
                continue;
            };
            if !body.enabled {
                continue;
            }
            if body.mode != PhysicsBodyMode::KinematicSlide {
                continue;
            }

            let Some(transform_cid) = world.parent_of(physics_cid) else {
                continue;
            };
            if world
                .get_component_by_id_as::<TransformComponent>(transform_cid)
                .is_none()
            {
                continue;
            }

            // Find a CollisionComponent that is a direct child of this transform.
            let mut collider_cid: Option<ComponentId> = None;
            for child in world.children_of(transform_cid) {
                if world
                    .get_component_by_id_as::<CollisionComponent>(*child)
                    .is_some()
                {
                    collider_cid = Some(*child);
                    break;
                }
            }
            let Some(collider_cid) = collider_cid else {
                continue;
            };

            // Gather static colliders overlapping this collider according to the CollisionSystem.
            let mut statics: Vec<ComponentId> = Vec::new();
            for (a, b) in pairs.iter().copied() {
                if a == collider_cid {
                    statics.push(b);
                } else if b == collider_cid {
                    statics.push(a);
                }
            }
            if statics.is_empty() {
                continue;
            }

            // Filter to static only.
            statics.retain(|&other| {
                world
                    .get_component_by_id_as::<CollisionComponent>(other)
                    .is_some_and(|c| c.mode == crate::engine::ecs::component::CollisionMode::Static)
            });
            if statics.is_empty() {
                continue;
            }

            // Resolve by repeated minimal-axis push-out.
            let mut moved = false;
            let mut desired_world_pos = match TransformSystem::world_position(world, transform_cid) {
                Some(p) => p,
                None => continue,
            };

            let a_shape = resolve_shape(world, collider_cid)
                .unwrap_or_else(|| crate::engine::ecs::component::CollisionShape::CUBE());

            for _ in 0..body.max_iterations {
                let mut any_overlap = false;

                // Resolve against each static collider one-by-one.
                for &static_cid in statics.iter() {
                    let Some(static_parent) = world.parent_of(static_cid) else {
                        continue;
                    };
                    // Static collision objects are also expected to be direct children of transforms.
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

                    let Some(push) = compute_push_out_aabb(
                        desired_world_pos,
                        a_shape,
                        b_world_pos,
                        b_shape,
                        body.push_out_epsilon,
                    ) else {
                        continue;
                    };

                    desired_world_pos[0] += push[0];
                    desired_world_pos[1] += push[1];
                    desired_world_pos[2] += push[2];
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

            // Convert desired world position into local translation for this TransformComponent.
            let new_local_translation = world_to_local_translation(world, transform_cid, desired_world_pos);

            if let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(transform_cid) {
                t.transform.translation = new_local_translation;
                t.transform.recompute_model();
                queue.queue_update_transform(transform_cid, t.transform);
            }
        }
    }
}

fn resolve_shape(world: &World, collision_cid: ComponentId) -> Option<CollisionShape> {
    // 1) Child CollisionShapeComponent.
    for child in world.children_of(collision_cid) {
        if let Some(s) = world.get_component_by_id_as::<CollisionShapeComponent>(*child) {
            return Some(s.shape);
        }
    }

    // 2) Sibling RenderableComponent with built-in mesh handles.
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

fn compute_push_out_aabb(
    a_center: [f32; 3],
    a_shape: CollisionShape,
    b_center: [f32; 3],
    b_shape: CollisionShape,
    eps: f32,
) -> Option<[f32; 3]> {
    let (a_min, a_max) = aabb_world(a_center, a_shape);
    let (b_min, b_max) = aabb_world(b_center, b_shape);

    let overlap_x = f32::min(a_max[0], b_max[0]) - f32::max(a_min[0], b_min[0]);
    let overlap_y = f32::min(a_max[1], b_max[1]) - f32::max(a_min[1], b_min[1]);
    let overlap_z = f32::min(a_max[2], b_max[2]) - f32::max(a_min[2], b_min[2]);

    if overlap_x <= 0.0 || overlap_y <= 0.0 || overlap_z <= 0.0 {
        return None;
    }

    // Choose minimum-penetration axis.
    let mut axis = 0;
    let mut min_overlap = overlap_x;
    if overlap_y < min_overlap {
        min_overlap = overlap_y;
        axis = 1;
    }
    if overlap_z < min_overlap {
        min_overlap = overlap_z;
        axis = 2;
    }

    let mut out = [0.0f32; 3];
    let dir = if a_center[axis] < b_center[axis] { -1.0 } else { 1.0 };
    let push = dir * (min_overlap + eps);
    out[axis] = push;
    Some(out)
}

fn aabb_world(center: [f32; 3], shape: CollisionShape) -> ([f32; 3], [f32; 3]) {
    match shape {
        CollisionShape::Cube { half_extents } => (
            [
                center[0] - half_extents[0],
                center[1] - half_extents[1],
                center[2] - half_extents[2],
            ],
            [
                center[0] + half_extents[0],
                center[1] + half_extents[1],
                center[2] + half_extents[2],
            ],
        ),
        CollisionShape::Sphere { radius } => (
            [center[0] - radius, center[1] - radius, center[2] - radius],
            [center[0] + radius, center[1] + radius, center[2] + radius],
        ),
    }
}

fn mat4_mul_vec4(m: [[f32; 4]; 4], v: [f32; 4]) -> [f32; 4] {
    // Column-major matrix-vector multiply.
    [
        m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2] + m[3][0] * v[3],
        m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2] + m[3][1] * v[3],
        m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2] + m[3][2] * v[3],
        m[0][3] * v[0] + m[1][3] * v[1] + m[2][3] * v[2] + m[3][3] * v[3],
    ]
}

fn world_to_local_translation(world: &World, transform_cid: ComponentId, desired_world: [f32; 3]) -> [f32; 3] {
    // If this transform has an ancestor transform, convert desired world position into local space.
    let mut cur = transform_cid;
    while let Some(parent) = world.parent_of(cur) {
        if let Some(t) = world.get_component_by_id_as::<TransformComponent>(parent) {
            if let Some(inv) = math::mat4_inverse(t.transform.matrix_world) {
                let p_local = mat4_mul_vec4(inv, [desired_world[0], desired_world[1], desired_world[2], 1.0]);
                return [p_local[0], p_local[1], p_local[2]];
            }
            break;
        }
        cur = parent;
    }

    // No transform parent; treat local == world.
    desired_world
}
