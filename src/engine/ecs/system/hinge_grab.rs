use crate::engine::ecs::component::{TransformComponent, TransformOperatorComponent};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::utils::math::{
    mat4_inverse, mat4_mul_vec4, quat_from_axis_angle, quat_rotate_vec3, vec3_cross, vec3_dot,
    vec3_len, vec3_normalize, vec3_sub,
};

use super::super::TransformSystem;
use super::super::raycast_system::PointerRaySnapshot;

/// The first operator before the next ancestor transform owns this grab mode.
pub(super) fn hinge_for_target(world: &World, target: ComponentId) -> Option<ComponentId> {
    let mut current = target;
    while let Some(parent) = world.parent_of(current) {
        if world
            .get_component_by_id_as::<TransformComponent>(parent)
            .is_some()
        {
            return None;
        }
        if let Some(operator) = world.get_component_by_id_as::<TransformOperatorComponent>(parent) {
            return operator.hinge_settings().map(|_| parent);
        }
        current = parent;
    }
    None
}

#[derive(Debug)]
pub(super) struct HingeGrab {
    pub target: ComponentId,
    pub hinge: ComponentId,
    pub raycaster: ComponentId,
    distance: f32,
    previous_radius: Option<[f32; 3]>,
}

impl HingeGrab {
    pub fn start(
        world: &World,
        target: ComponentId,
        hinge: ComponentId,
        raycaster: ComponentId,
        ray: PointerRaySnapshot,
    ) -> Option<Self> {
        let origin = TransformSystem::world_position(world, target)?;
        let distance = vec3_dot(vec3_sub(origin, ray.origin_world), ray.direction_world).max(0.0);
        let mut grab = Self {
            target,
            hinge,
            raycaster,
            distance,
            previous_radius: None,
        };
        grab.previous_radius = grab.radius(world, ray);
        Some(grab)
    }

    fn radius(&self, world: &World, ray: PointerRaySnapshot) -> Option<[f32; 3]> {
        let hinge = world
            .get_component_by_id_as::<TransformOperatorComponent>(self.hinge)?
            .hinge_settings()?;
        let target = &world
            .get_component_by_id_as::<TransformComponent>(self.target)?
            .transform;
        let point = std::array::from_fn::<_, 3, _>(|i| {
            ray.origin_world[i] + self.distance * ray.direction_world[i]
        });
        let [x, y, z, w] = mat4_mul_vec4(
            mat4_inverse(target.matrix_world)?,
            [point[0], point[1], point[2], 1.0],
        );
        // Recover the unrotated hinge frame from the current driven transform.
        let local = TransformSystem::effective_local_model(world, self.target, target.model);
        let [x, y, z, _] = mat4_mul_vec4(local, [x, y, z, w]);
        let axis = hinge.axis();
        let point = quat_rotate_vec3(quat_from_axis_angle(axis, hinge.angle()), [x, y, z]);
        let axial = vec3_dot(point, axis);
        let radius = std::array::from_fn(|i| point[i] - axial * axis[i]);
        let length = vec3_len(radius);
        (length.is_finite() && length > 1e-4).then(|| vec3_normalize(radius))
    }

    pub fn update(
        &mut self,
        world: &mut World,
        ray: Option<PointerRaySnapshot>,
        emit: &mut dyn SignalEmitter,
    ) -> bool {
        let Some(current) = ray.and_then(|ray| self.radius(world, ray)) else {
            self.previous_radius = None;
            return false;
        };
        let Some(previous) = self.previous_radius.replace(current) else {
            return false;
        };
        let Some(hinge) = world
            .get_component_by_id_as_mut::<TransformOperatorComponent>(self.hinge)
            .and_then(|operator| operator.hinge_settings_mut())
        else {
            return false;
        };
        let delta = vec3_dot(hinge.axis(), vec3_cross(previous, current))
            .atan2(vec3_dot(previous, current));
        if delta.abs() < 1e-6 || !hinge.set_angle(hinge.angle() + delta).unwrap_or(false) {
            return false;
        }
        emit.push_intent_now(
            self.hinge,
            IntentValue::UpdateTransformWorld {
                component_id: self.hinge,
            },
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::PointerComponent;
    use crate::engine::ecs::system::{RayCastSystem, SystemWorld};
    use crate::engine::ecs::{CommandQueue, EventSignal, Signal};
    use crate::engine::graphics::{RenderAssets, VisualWorld};
    use std::f32::consts::FRAC_PI_2;

    #[test]
    fn grabbable_drives_hinge_without_reparenting_and_tracks_base_motion() {
        let mut world = World::default();
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let assets = RenderAssets::new();
        let mut queue = CommandQueue::new();
        let base = world.add_component(TransformComponent::new());
        let mut hinge = TransformOperatorComponent::hinge([1.0, 0.0, 0.0]).unwrap();
        hinge
            .hinge_settings_mut()
            .unwrap()
            .set_limits(0.0, FRAC_PI_2)
            .unwrap();
        let hinge = world.add_component(hinge);
        let mut arm = TransformComponent::new();
        arm.transform.translation = [0.0, 0.0, 1.0];
        arm.transform.recompute_model();
        let arm = world.add_component(arm);
        let pointer = world.add_component(PointerComponent::new());
        world.add_child(base, hinge).unwrap();
        world.add_child(hinge, arm).unwrap();
        systems.transform_changed(&mut world, &mut visuals, base);
        systems.grabbable.install_handlers(&mut systems.rx);

        let raycaster = ComponentId::default();
        let direction_world = [1.0, 0.0, 0.0];
        systems.rx.dispatch_event_handlers(
            &mut world,
            &Signal::event(
                arm,
                EventSignal::GrabStart {
                    pointer,
                    target: arm,
                    renderable: arm,
                    raycaster,
                    ray_origin_world: [-1.0, 0.0, 1.0],
                    ray_dir_world: direction_world,
                },
            ),
        );
        systems.grabbable.tick(&mut world, &assets, &mut queue, 0.0);
        assert_eq!(world.parent_of(arm), Some(hinge));

        let base_transform = &mut world
            .get_component_by_id_as_mut::<TransformComponent>(base)
            .unwrap()
            .transform;
        base_transform.translation = [3.0, 4.0, 5.0];
        base_transform.recompute_model();
        systems.transform_changed(&mut world, &mut visuals, base);
        let mut raycasts = RayCastSystem::default();
        raycasts.set_current_ray_for_test(
            raycaster,
            PointerRaySnapshot {
                origin_world: [2.0, 4.0, 6.0],
                direction_world,
            },
        );
        assert!(
            !systems
                .grabbable
                .tick_hinge_grabs(&mut world, &raycasts, &mut queue)
        );
        raycasts.set_current_ray_for_test(
            raycaster,
            PointerRaySnapshot {
                origin_world: [2.0, 3.0, 4.0],
                direction_world,
            },
        );
        assert!(
            systems
                .grabbable
                .tick_hinge_grabs(&mut world, &raycasts, &mut queue)
        );
        systems.transform_changed(&mut world, &mut visuals, hinge);
        let [x, y, z] = TransformSystem::world_position(&world, arm).unwrap();
        assert!((x - 3.0).abs() < 1e-5 && (y - 3.0).abs() < 1e-5 && (z - 5.0).abs() < 1e-5);

        systems.rx.dispatch_event_handlers(
            &mut world,
            &Signal::event(
                arm,
                EventSignal::GrabEnd {
                    pointer,
                    target: arm,
                },
            ),
        );
        systems.grabbable.tick(&mut world, &assets, &mut queue, 0.0);
        assert_eq!(world.parent_of(arm), Some(hinge));
        assert!(
            !systems
                .grabbable
                .tick_hinge_grabs(&mut world, &raycasts, &mut queue)
        );
    }
}
