use crate::engine::ecs::component::{
    GrabbableComponent, PointerComponent, RaycastableComponent, SerializeComponent,
    TransformComponent,
};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::system::bounds_system::BoundsSystem;
use crate::engine::ecs::system::pointer_system::{
    nearest_ancestor_transform, pointer_topology_context,
};
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, RxWorld, SignalEmitter, SignalKind, World,
};
use crate::engine::graphics::RenderAssets;
use crate::utils::math::{mat_to_quat, mat4_inverse, mat4_mul, vec3_dot, vec3_len, vec3_normalize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy)]
enum GrabCommand {
    Start {
        pointer: ComponentId,
        target: ComponentId,
        ray_origin: [f32; 3],
        ray_dir: [f32; 3],
    },
    End {
        pointer: ComponentId,
    },
}

#[derive(Debug, Clone, Copy)]
struct ActiveGrab {
    target: ComponentId,
    pointer_parent: ComponentId,
    original_parent: Option<ComponentId>,
    destination_local: [f32; 3],
}

#[derive(Debug, Default)]
pub struct GrabbableSystem {
    handlers_installed: bool,
    commands: Arc<Mutex<Vec<GrabCommand>>>,
    active: HashMap<ComponentId, ActiveGrab>,
    owned: HashSet<ComponentId>,
}

impl GrabbableSystem {
    pub fn install_handlers(&mut self, rx: &mut RxWorld) {
        if self.handlers_installed {
            return;
        }
        let starts = self.commands.clone();
        rx.add_global_handler_closure(SignalKind::GrabStart, move |_world, _emit, env| {
            let Some(EventSignal::GrabStart {
                pointer,
                target,
                ray_origin_world,
                ray_dir_world,
                ..
            }) = env.event.as_ref()
            else {
                return;
            };
            if let Ok(mut commands) = starts.lock() {
                commands.push(GrabCommand::Start {
                    pointer: *pointer,
                    target: *target,
                    ray_origin: *ray_origin_world,
                    ray_dir: *ray_dir_world,
                });
            }
        });
        let ends = self.commands.clone();
        rx.add_global_handler_closure(SignalKind::GrabEnd, move |_world, _emit, env| {
            let Some(EventSignal::GrabEnd { pointer, .. }) = env.event.as_ref() else {
                return;
            };
            if let Ok(mut commands) = ends.lock() {
                commands.push(GrabCommand::End { pointer: *pointer });
            }
        });
        self.handlers_installed = true;
    }

    pub fn register(
        &mut self,
        world: &mut World,
        grabbable: ComponentId,
        emit: &mut dyn SignalEmitter,
    ) {
        if world
            .get_component_by_id_as::<GrabbableComponent>(grabbable)
            .is_some_and(|c| !c.enabled)
        {
            return;
        }
        let Some(owner) = world.parent_of(grabbable).filter(|id| {
            world
                .get_component_by_id_as::<TransformComponent>(*id)
                .is_some()
        }) else {
            return;
        };
        ensure_generated_raycastable(world, owner, "grabbable_generated_raycastable", emit);
    }

    pub fn tick(
        &mut self,
        world: &mut World,
        render_assets: &RenderAssets,
        emit: &mut dyn SignalEmitter,
        dt: f32,
    ) {
        let commands = self
            .commands
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default();
        for command in commands {
            match command {
                GrabCommand::Start {
                    pointer,
                    target,
                    ray_origin,
                    ray_dir,
                } => self.start(
                    world,
                    render_assets,
                    emit,
                    pointer,
                    target,
                    ray_origin,
                    ray_dir,
                ),
                GrabCommand::End { pointer } => self.release(world, emit, pointer),
            }
        }

        let invalid: Vec<_> = self
            .active
            .iter()
            .filter_map(|(&pointer, grab)| {
                let valid = world.get_component_record(pointer).is_some()
                    && world
                        .get_component_by_id_as::<TransformComponent>(grab.target)
                        .is_some()
                    && world.get_component_record(grab.pointer_parent).is_some()
                    && world.parent_of(grab.target) == Some(grab.pointer_parent);
                (!valid).then_some(pointer)
            })
            .collect();
        for pointer in invalid {
            self.release(world, emit, pointer);
        }

        let alpha = 1.0 - (-12.0 * dt.max(0.0)).exp();
        for grab in self.active.values() {
            let Some(transform) =
                world.get_component_by_id_as_mut::<TransformComponent>(grab.target)
            else {
                continue;
            };
            let current = transform.transform.translation;
            let mut next = [0.0; 3];
            for axis in 0..3 {
                next[axis] = current[axis] + (grab.destination_local[axis] - current[axis]) * alpha;
            }
            if vec3_len([
                next[0] - grab.destination_local[0],
                next[1] - grab.destination_local[1],
                next[2] - grab.destination_local[2],
            ]) <= 0.001
            {
                next = grab.destination_local;
            }
            transform.transform.translation = next;
            transform.transform.recompute_model();
            emit.push_intent_now(
                grab.target,
                IntentValue::UpdateTransform {
                    component_ids: vec![grab.target],
                    translation: next,
                    rotation_quat_xyzw: transform.transform.rotation,
                    scale: transform.transform.scale,
                },
            );
        }
    }

    fn start(
        &mut self,
        world: &mut World,
        render_assets: &RenderAssets,
        emit: &mut dyn SignalEmitter,
        pointer: ComponentId,
        target: ComponentId,
        ray_origin: [f32; 3],
        ray_dir: [f32; 3],
    ) {
        if self.active.contains_key(&pointer) || self.owned.contains(&target) {
            return;
        }
        let Some(pointer_parent) = world
            .parent_of(pointer)
            .and_then(|p| nearest_ancestor_transform(world, p))
        else {
            return;
        };
        let Some(target_world) = TransformSystem::world_model(world, target) else {
            return;
        };
        let original_parent = world.parent_of(target);
        let dir = vec3_normalize(ray_dir);
        if !dir.iter().all(|v| v.is_finite()) {
            return;
        }

        let target_origin = [target_world[3][0], target_world[3][1], target_world[3][2]];
        let along = vec3_dot(
            [
                target_origin[0] - ray_origin[0],
                target_origin[1] - ray_origin[1],
                target_origin[2] - ray_origin[2],
            ],
            dir,
        );
        let transverse = [
            target_origin[0] - ray_origin[0] - dir[0] * along,
            target_origin[1] - ray_origin[1] - dir[1] * along,
            target_origin[2] - ray_origin[2] - dir[2] * along,
        ];
        let min_projection =
            BoundsSystem::calculate_subtree_local_bounds(world, render_assets, target)
                .map(|bounds| {
                    let mut min = f32::INFINITY;
                    for x in [bounds.min[0], bounds.max[0]] {
                        for y in [bounds.min[1], bounds.max[1]] {
                            for z in [bounds.min[2], bounds.max[2]] {
                                let p = transform_point(target_world, [x, y, z]);
                                min = min.min(vec3_dot(
                                    [
                                        p[0] - target_origin[0],
                                        p[1] - target_origin[1],
                                        p[2] - target_origin[2],
                                    ],
                                    dir,
                                ));
                            }
                        }
                    }
                    min
                })
                .unwrap_or(0.0);
        let clearance = pointer_grab_distance(world, pointer);
        let distance = clearance - min_projection;
        let desired_world = [
            ray_origin[0] + dir[0] * distance + transverse[0],
            ray_origin[1] + dir[1] * distance + transverse[1],
            ray_origin[2] + dir[2] * distance + transverse[2],
        ];

        if reparent_preserving_world(world, target, Some(pointer_parent), emit).is_err() {
            return;
        }
        let Some(parent_world_inv) =
            TransformSystem::world_model(world, pointer_parent).and_then(mat4_inverse)
        else {
            let _ = reparent_preserving_world(world, target, original_parent, emit);
            return;
        };
        let destination_local = transform_point(parent_world_inv, desired_world);
        self.owned.insert(target);
        self.active.insert(
            pointer,
            ActiveGrab {
                target,
                pointer_parent,
                original_parent,
                destination_local,
            },
        );
    }

    fn release(&mut self, world: &mut World, emit: &mut dyn SignalEmitter, pointer: ComponentId) {
        let Some(grab) = self.active.remove(&pointer) else {
            return;
        };
        self.owned.remove(&grab.target);
        if world.get_component_record(grab.target).is_none() {
            return;
        }
        let parent = grab
            .original_parent
            .filter(|p| world.get_component_record(*p).is_some());
        if reparent_preserving_world(world, grab.target, parent, emit).is_err() {
            let _ = reparent_preserving_world(world, grab.target, None, emit);
        }
    }
}

pub fn pointer_grab_distance(world: &World, pointer: ComponentId) -> f32 {
    if let Some(value) = world
        .get_component_by_id_as::<PointerComponent>(pointer)
        .and_then(|p| p.min_grab_distance)
    {
        return value;
    }
    let topology = pointer_topology_context(world, pointer);
    if topology.has_controller_driver {
        0.05
    } else {
        0.75
    }
}

pub fn grabbable_owner_for_hit(world: &World, renderable: ComponentId) -> Option<ComponentId> {
    resolve_marker_target::<GrabbableComponent>(world, renderable, |c| (c.enabled, c.move_parent))
}

pub(crate) fn resolve_marker_target<T: 'static>(
    world: &World,
    renderable: ComponentId,
    fields: impl Fn(&T) -> (bool, bool),
) -> Option<ComponentId> {
    let mut current = Some(renderable);
    while let Some(id) = current {
        if world
            .get_component_by_id_as::<TransformComponent>(id)
            .is_some()
        {
            if let Some(marker) = world
                .children_of(id)
                .iter()
                .find_map(|c| world.get_component_by_id_as::<T>(*c))
            {
                let (enabled, move_parent) = fields(marker);
                if !enabled {
                    return None;
                }
                if !move_parent {
                    return Some(id);
                }
                let mut parent = world.parent_of(id);
                while let Some(candidate) = parent {
                    if world
                        .get_component_by_id_as::<TransformComponent>(candidate)
                        .is_some()
                    {
                        return Some(candidate);
                    }
                    parent = world.parent_of(candidate);
                }
                return None;
            }
        }
        current = world.parent_of(id);
    }
    None
}

pub(crate) fn ensure_generated_raycastable(
    world: &mut World,
    owner: ComponentId,
    label: &str,
    emit: &mut dyn SignalEmitter,
) {
    if world.children_of(owner).iter().any(|c| {
        world
            .get_component_by_id_as::<RaycastableComponent>(*c)
            .is_some()
    }) {
        return;
    }
    let raycastable =
        world.add_component_boxed_named(label, Box::new(RaycastableComponent::enabled()));
    let serialize = world.add_component(SerializeComponent::off());
    let _ = world.add_child(raycastable, serialize);
    if world.add_child(owner, raycastable).is_ok() {
        world.init_component_tree(raycastable, emit);
    }
}

/// Reparent a Transform while keeping its current world-space pose.
pub fn reparent_preserving_world(
    world: &mut World,
    child: ComponentId,
    new_parent: Option<ComponentId>,
    emit: &mut dyn SignalEmitter,
) -> Result<(), &'static str> {
    let world_matrix =
        TransformSystem::world_model(world, child).ok_or("child has no world transform")?;
    let old_parent = world.parent_of(child);
    let local = if let Some(parent) = new_parent {
        let parent_world =
            TransformSystem::world_model(world, parent).ok_or("parent has no world transform")?;
        mat4_mul(
            mat4_inverse(parent_world).ok_or("parent transform is singular")?,
            world_matrix,
        )
    } else {
        world_matrix
    };
    world.set_parent(child, new_parent)?;
    let (translation, rotation, scale) = decompose(local);
    let transform = world
        .get_component_by_id_as_mut::<TransformComponent>(child)
        .ok_or("child is not a transform")?;
    transform.transform.translation = translation;
    transform.transform.rotation = rotation;
    transform.transform.scale = scale;
    transform.transform.recompute_model();
    transform.transform.matrix_world = world_matrix;
    emit.push_event(
        child,
        EventSignal::ParentChanged {
            child,
            old_parent,
            new_parent,
        },
    );
    emit.push_intent_now(
        child,
        IntentValue::UpdateTransform {
            component_ids: vec![child],
            translation,
            rotation_quat_xyzw: rotation,
            scale,
        },
    );
    if let Some(parent) = old_parent {
        emit.push_intent_now(
            parent,
            IntentValue::AudioGraphDirtyImmediate {
                component_ids: vec![parent],
            },
        );
    }
    if let Some(parent) = new_parent {
        emit.push_intent_now(
            parent,
            IntentValue::AudioGraphDirtyImmediate {
                component_ids: vec![parent],
            },
        );
    }
    Ok(())
}

fn decompose(m: [[f32; 4]; 4]) -> ([f32; 3], [f32; 4], [f32; 3]) {
    let translation = [m[3][0], m[3][1], m[3][2]];
    let scale = [
        vec3_len([m[0][0], m[0][1], m[0][2]]).max(1e-8),
        vec3_len([m[1][0], m[1][1], m[1][2]]).max(1e-8),
        vec3_len([m[2][0], m[2][1], m[2][2]]).max(1e-8),
    ];
    let mut rotation_matrix = m;
    for c in 0..3 {
        for r in 0..3 {
            rotation_matrix[c][r] /= scale[c];
        }
    }
    (translation, mat_to_quat(rotation_matrix), scale)
}

fn transform_point(m: [[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0],
        m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1],
        m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::component::{
        ControllerHand, ControllerPoseKind, ControllerXRComponent, RenderableComponent,
    };

    fn sync_world(world: &mut World, id: ComponentId, parent_world: Option<[[f32; 4]; 4]>) {
        let local = world
            .get_component_by_id_as::<TransformComponent>(id)
            .unwrap()
            .transform
            .model;
        let matrix = parent_world.map(|p| mat4_mul(p, local)).unwrap_or(local);
        world
            .get_component_by_id_as_mut::<TransformComponent>(id)
            .unwrap()
            .transform
            .matrix_world = matrix;
    }

    #[test]
    fn reparent_preserves_world_pose_across_rotated_scaled_parents() {
        let mut world = World::default();
        let old = world.add_component(
            TransformComponent::new()
                .with_position(3.0, 1.0, -2.0)
                .with_scale(2.0, 2.0, 2.0),
        );
        let new = world.add_component(
            TransformComponent::new()
                .with_position(-4.0, 2.0, 1.0)
                .with_rotation_euler(0.0, 0.7, 0.0),
        );
        let child = world.add_component(TransformComponent::new().with_position(0.5, 1.0, -0.25));
        world.add_child(old, child).unwrap();
        sync_world(&mut world, old, None);
        sync_world(&mut world, new, None);
        let old_world = TransformSystem::world_model(&world, old).unwrap();
        sync_world(&mut world, child, Some(old_world));
        let before = TransformSystem::world_model(&world, child).unwrap();
        let mut queue = CommandQueue::new();
        reparent_preserving_world(&mut world, child, Some(new), &mut queue).unwrap();
        assert_eq!(world.parent_of(child), Some(new));
        assert_eq!(TransformSystem::world_model(&world, child), Some(before));
        reparent_preserving_world(&mut world, child, Some(old), &mut queue).unwrap();
        assert_eq!(world.parent_of(child), Some(old));
        assert_eq!(TransformSystem::world_model(&world, child), Some(before));
    }

    #[test]
    fn pointer_clearance_uses_topology_defaults_and_override() {
        let mut world = World::default();
        let desktop = world.add_component(PointerComponent::new());
        assert_eq!(pointer_grab_distance(&world, desktop), 0.75);
        let controller = world.add_component(ControllerXRComponent::new(
            true,
            ControllerHand::Left,
            ControllerPoseKind::Grip,
        ));
        let xr_pointer = world.add_component(PointerComponent::new());
        world.add_child(controller, xr_pointer).unwrap();
        assert_eq!(pointer_grab_distance(&world, xr_pointer), 0.05);
        world
            .get_component_by_id_as_mut::<PointerComponent>(xr_pointer)
            .unwrap()
            .min_grab_distance = Some(0.2);
        assert_eq!(pointer_grab_distance(&world, xr_pointer), 0.2);
    }

    #[test]
    fn unmeasurable_target_eases_its_origin_to_clearance() {
        let mut world = World::default();
        let driver = world.add_component(TransformComponent::new());
        let pointer = world.add_component(PointerComponent::new().min_grab_distance(0.25));
        let target = world.add_component(TransformComponent::new().with_position(0.0, 0.0, -2.0));
        world.add_child(driver, pointer).unwrap();
        sync_world(&mut world, driver, None);
        sync_world(&mut world, target, None);
        let mut system = GrabbableSystem::default();
        let mut queue = CommandQueue::new();
        system.start(
            &mut world,
            &RenderAssets::new(),
            &mut queue,
            pointer,
            target,
            [0.0; 3],
            [0.0, 0.0, -1.0],
        );
        assert_eq!(world.parent_of(target), Some(driver));
        system.tick(&mut world, &RenderAssets::new(), &mut queue, 10.0);
        let position = world
            .get_component_by_id_as::<TransformComponent>(target)
            .unwrap()
            .transform
            .translation;
        assert!((position[2] + 0.25).abs() < 0.001);
    }

    #[test]
    fn primitive_bounds_keep_ray_facing_surface_at_clearance() {
        let mut world = World::default();
        let driver = world.add_component(TransformComponent::new());
        let pointer = world.add_component(PointerComponent::new().min_grab_distance(0.1));
        let target = world.add_component(TransformComponent::new().with_position(0.0, 0.0, -2.0));
        let cube = world.add_component(RenderableComponent::cube());
        world.add_child(driver, pointer).unwrap();
        world.add_child(target, cube).unwrap();
        sync_world(&mut world, driver, None);
        sync_world(&mut world, target, None);
        let mut system = GrabbableSystem::default();
        let mut queue = CommandQueue::new();
        system.start(
            &mut world,
            &RenderAssets::new(),
            &mut queue,
            pointer,
            target,
            [0.0; 3],
            [0.0, 0.0, -1.0],
        );
        system.tick(&mut world, &RenderAssets::new(), &mut queue, 10.0);
        let z = world
            .get_component_by_id_as::<TransformComponent>(target)
            .unwrap()
            .transform
            .translation[2];
        assert!(
            (z + 0.6).abs() < 0.001,
            "center should be clearance plus cube half extent: {z}"
        );
    }
}
