use std::collections::HashSet;

use crate::engine::ecs::component::{
    GravityComponent, InputXRComponent, QueryRootMode, TransformComponent, VelocityComponent,
    resolve_component_ref,
};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::{CameraTarget, VisualWorld};
use crate::utils::math;

const STEP_SEC: f64 = 1.0 / 120.0;
const MAX_STEPS: usize = 8;

#[derive(Debug, Default)]
pub struct VelocitySystem {
    accumulator_sec: f64,
    dropped_since_report_sec: f64,
    reported_bad_target: HashSet<ComponentId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::RxWorld;
    use crate::engine::ecs::system::{
        CameraSystem, CollisionSystem, LightSystem, TransformStreamSystem,
    };

    #[test]
    fn gravity_provider_respects_branches_overrides_and_velocity_boundaries() {
        let mut world = World::default();
        let gravity = world.add_component(GravityComponent::new());
        let frame = world.add_component(TransformComponent::new());
        let outer = world.add_component(VelocityComponent::new());
        let target = world.add_component(TransformComponent::new());
        let inner = world.add_component(VelocityComponent::new());
        let off = world.add_component(GravityComponent::off());
        let body = world.add_component(VelocityComponent::new());
        let sibling = world.add_component(VelocityComponent::new());
        for (parent, child) in [
            (gravity, frame),
            (frame, outer),
            (outer, target),
            (target, inner),
            (target, off),
            (off, body),
            (frame, sibling),
        ] {
            world.add_child(parent, child).unwrap();
        }
        assert_eq!(
            VelocitySystem::gravity_provider(&world, outer),
            Some(gravity)
        );
        assert_eq!(
            VelocitySystem::gravity_provider(&world, sibling),
            Some(gravity)
        );
        assert_eq!(VelocitySystem::gravity_provider(&world, inner), None);
        assert_eq!(VelocitySystem::gravity_provider(&world, body), Some(off));
        world
            .get_component_by_id_as_mut::<VelocityComponent>(outer)
            .unwrap()
            .enabled = false;
        assert_eq!(VelocitySystem::gravity_provider(&world, inner), None);
        world
            .get_component_by_id_as_mut::<VelocityComponent>(outer)
            .unwrap()
            .enabled = true;
        assert_eq!(VelocitySystem::gravity_provider(&world, inner), None);
        // A nearer disabled provider blocks the enabled provider in the same scope.
        world.add_child(frame, off).unwrap();
        assert_eq!(VelocitySystem::gravity_provider(&world, body), Some(off));
        world
            .get_component_by_id_as_mut::<GravityComponent>(off)
            .unwrap()
            .enabled = true;
        assert_eq!(VelocitySystem::gravity_provider(&world, body), Some(off));
    }

    #[test]
    fn gravity_provider_tracks_reparenting_removal_and_rejects_child_providers() {
        let mut world = World::default();
        let gravity = world.add_component(GravityComponent::new());
        let velocity = world.add_component(VelocityComponent::new());
        let child = world.add_component(GravityComponent::new());
        world.add_child(velocity, child).unwrap();
        assert_eq!(VelocitySystem::gravity_provider(&world, velocity), None);
        assert_eq!(VelocitySystem::gravity_provider(&world, child), None);
        world.add_child(gravity, velocity).unwrap();
        assert_eq!(
            VelocitySystem::gravity_provider(&world, velocity),
            Some(gravity)
        );
        world.detach_from_parent(velocity);
        world.remove_component_leaf(gravity).unwrap();
        assert_eq!(VelocitySystem::gravity_provider(&world, velocity), None);
    }

    fn propagate(world: &mut World, root: ComponentId) {
        TransformSystem::new().transform_changed(
            world,
            &mut VisualWorld::default(),
            root,
            &mut TransformStreamSystem::new(),
            &mut CameraSystem::new(),
            &mut LightSystem::new(),
            &mut CollisionSystem::new(),
        );
    }

    #[test]
    fn gravity_is_world_down_under_rotated_scaled_parent_and_horizontal_velocity() {
        let mut world = World::default();
        let gravity = world.add_component(GravityComponent::new());
        let frame = world.add_component(
            TransformComponent::new()
                .with_rotation_euler(0.0, 0.0, std::f32::consts::FRAC_PI_2)
                .with_scale(2.0, 3.0, 4.0),
        );
        let mut state = VelocityComponent::new();
        state.horizontal = true;
        let owner = world.add_component(state);
        let target = world.add_component(TransformComponent::new());
        world.add_child(gravity, frame).unwrap();
        world.add_child(frame, owner).unwrap();
        world.add_child(owner, target).unwrap();
        propagate(&mut world, frame);
        let mut system = VelocitySystem::default();
        system.step(&mut world, &mut RxWorld::default());
        propagate(&mut world, frame);
        let position = TransformSystem::world_position(&world, target).unwrap();
        assert!(position[0].abs() < 1.0e-6);
        assert!((position[1] + 9.81 / 120.0 / 120.0).abs() < 1.0e-6);
    }

    #[test]
    fn gravity_edits_disables_and_nested_boundaries_affect_live_velocity() {
        let mut world = World::default();
        let gravity = world.add_component(GravityComponent::new());
        let owner = world.add_component(VelocityComponent::new());
        let target = world.add_component(TransformComponent::new());
        let inner = world.add_component(VelocityComponent::new());
        let inner_target = world.add_component(TransformComponent::new());
        world.add_child(gravity, owner).unwrap();
        world.add_child(owner, target).unwrap();
        world.add_child(target, inner).unwrap();
        world.add_child(inner, inner_target).unwrap();
        propagate(&mut world, target);
        let mut system = VelocitySystem::default();
        let mut emit = RxWorld::default();
        system.step(&mut world, &mut emit);
        let speed = world
            .get_component_by_id_as::<VelocityComponent>(owner)
            .unwrap()
            .linear_local_mps;
        assert!((speed[1] + 9.81 / 120.0).abs() < 1.0e-6);
        assert_eq!(
            world
                .get_component_by_id_as::<VelocityComponent>(inner)
                .unwrap()
                .linear_local_mps,
            [0.0; 3]
        );
        world
            .get_component_by_id_as_mut::<GravityComponent>(gravity)
            .unwrap()
            .enabled = false;
        system.step(&mut world, &mut emit);
        assert_eq!(
            world
                .get_component_by_id_as::<VelocityComponent>(owner)
                .unwrap()
                .linear_local_mps,
            speed
        );
        let provider = world
            .get_component_by_id_as_mut::<GravityComponent>(gravity)
            .unwrap();
        provider.enabled = true;
        provider.set_coefficient(-1.0).unwrap();
        assert!(provider.set_coefficient(f32::NAN).is_err());
        assert_eq!(provider.coefficient, -1.0);
        world
            .get_component_by_id_as_mut::<VelocityComponent>(owner)
            .unwrap()
            .enabled = false;
        let before = world
            .get_component_by_id_as::<TransformComponent>(target)
            .unwrap()
            .transform
            .translation;
        system.tick(&mut world, &mut emit, 1.0 / 30.0);
        assert_eq!(
            world
                .get_component_by_id_as::<TransformComponent>(target)
                .unwrap()
                .transform
                .translation,
            before
        );
        world
            .get_component_by_id_as_mut::<VelocityComponent>(owner)
            .unwrap()
            .enabled = true;
        system.step(&mut world, &mut emit);
        assert!(
            world
                .get_component_by_id_as::<VelocityComponent>(owner)
                .unwrap()
                .linear_local_mps[1]
                .abs()
                < 1.0e-6
        );
    }

    #[test]
    fn local_clicks_add_world_velocity_once_and_back_cancels() {
        let mut world = World::default();
        let root = world.add_component(TransformComponent::new().with_rotation_quat([
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
        ]));
        let velocity = world.add_component(VelocityComponent::new());
        let target = world.add_component(TransformComponent::new());
        world.add_child(root, velocity).unwrap();
        world.add_child(velocity, target).unwrap();
        propagate(&mut world, root);
        let mut system = VelocitySystem::default();
        let visuals = VisualWorld::default();
        system
            .translate(&mut world, &visuals, velocity, [0.0, 0.0, -0.25], false)
            .unwrap();
        let state = world
            .get_component_by_id_as::<VelocityComponent>(velocity)
            .unwrap();
        assert!(state.linear_local_mps[0].abs() < 1e-5);
        assert!((state.linear_local_mps[2] + 0.25).abs() < 1e-5);
        system
            .translate(&mut world, &visuals, velocity, [0.0, 0.0, 0.25], false)
            .unwrap();
        assert_eq!(
            world
                .get_component_by_id_as::<VelocityComponent>(velocity)
                .unwrap()
                .linear_local_mps,
            [0.0; 3]
        );
    }

    #[test]
    fn integration_preserves_metre_speed_through_scaled_parent() {
        let mut world = World::default();
        let parent = world.add_component(
            TransformComponent::new()
                .with_rotation_quat([
                    0.0,
                    std::f32::consts::FRAC_1_SQRT_2,
                    0.0,
                    std::f32::consts::FRAC_1_SQRT_2,
                ])
                .with_scale(2.0, 3.0, 4.0),
        );
        let root = world.add_component(TransformComponent::new());
        let mut component = VelocityComponent::new();
        component.linear_local_mps = [0.0, 0.0, 1.0];
        let velocity = world.add_component(component);
        world.add_child(parent, root).unwrap();
        world.add_child(root, velocity).unwrap();
        let target = world.add_component(TransformComponent::new());
        world.add_child(velocity, target).unwrap();
        propagate(&mut world, parent);
        let before = TransformSystem::world_position(&world, target).unwrap();
        let mut system = VelocitySystem::default();
        let mut rx = RxWorld::default();
        system.tick(&mut world, &mut rx, 1.0 / 60.0);
        propagate(&mut world, parent);
        let after = TransformSystem::world_position(&world, target).unwrap();
        assert!((after[0] - before[0] - 1.0 / 60.0).abs() < 1e-5);
        assert!((after[1] - before[1]).abs() < 1e-5);
        assert!((after[2] - before[2]).abs() < 1e-5);
    }

    #[test]
    fn equal_elapsed_time_at_two_frame_rates_matches_and_disable_stops_writes() {
        fn simulate(frames: usize, dt: f32) -> f32 {
            let mut world = World::default();
            let root = world.add_component(TransformComponent::new());
            let mut state = VelocityComponent::new();
            state.set_linear_local([0.0, 0.0, -1.0]).unwrap();
            let velocity = world.add_component(state);
            world.add_child(root, velocity).unwrap();
            let target = world.add_component(TransformComponent::new());
            world.add_child(velocity, target).unwrap();
            let mut system = VelocitySystem::default();
            let mut rx = RxWorld::default();
            for _ in 0..frames {
                system.tick(&mut world, &mut rx, dt);
            }
            let before_disable = world
                .get_component_by_id_as::<TransformComponent>(target)
                .unwrap()
                .translation()[2];
            world
                .get_component_by_id_as_mut::<VelocityComponent>(velocity)
                .unwrap()
                .enabled = false;
            system.tick(&mut world, &mut rx, dt);
            assert_eq!(
                world
                    .get_component_by_id_as::<TransformComponent>(target)
                    .unwrap()
                    .translation()[2],
                before_disable
            );
            before_disable
        }
        let at_60_hz = simulate(60, 1.0 / 60.0);
        let at_120_hz = simulate(120, 1.0 / 120.0);
        assert!((at_60_hz + 1.0).abs() < 1e-4);
        assert!((at_120_hz + 1.0).abs() < 1e-4);
        assert!((at_60_hz - at_120_hz).abs() < 1e-4);
    }

    #[test]
    fn invalid_delta_and_missing_xr_pose_leave_velocity_unchanged() {
        let mut world = World::default();
        let root = world.add_component(TransformComponent::new());
        let input = world.add_component(InputXRComponent::on());
        let input_guid = world.get_component_record(input).unwrap().guid;
        let mut component = VelocityComponent::new();
        component.rotation_basis = Some(crate::engine::ecs::component::ComponentRef::Guid(
            input_guid,
        ));
        let velocity = world.add_component(component);
        world.add_child(root, velocity).unwrap();
        let target = world.add_component(TransformComponent::new());
        world.add_child(velocity, target).unwrap();
        world.add_child(target, input).unwrap();
        propagate(&mut world, root);
        let mut system = VelocitySystem::default();
        let visuals = VisualWorld::default();
        assert!(
            system
                .translate(&mut world, &visuals, velocity, [f32::NAN, 0.0, 0.0], false)
                .is_err()
        );
        assert!(
            system
                .translate(&mut world, &visuals, velocity, [0.0, 0.0, -0.25], false)
                .is_err()
        );
        assert_eq!(
            world
                .get_component_by_id_as::<VelocityComponent>(velocity)
                .unwrap()
                .linear_local_mps,
            [0.0; 3]
        );
    }

    #[test]
    fn referenced_xr_eye_yaw_supplies_horizontal_delta() {
        use crate::engine::ecs::component::CameraXRComponent;
        use crate::engine::graphics::primitives::Transform;

        let mut world = World::default();
        let root = world.add_component(TransformComponent::new());
        let input = world.add_component(InputXRComponent::on());
        let camera = world.add_component(CameraXRComponent::on());
        let guid = world.get_component_record(input).unwrap().guid;
        let mut config = VelocityComponent::new();
        config.rotation_basis = Some(crate::engine::ecs::component::ComponentRef::Guid(guid));
        config.horizontal = true;
        let velocity = world.add_component(config);
        world.add_child(root, velocity).unwrap();
        let target = world.add_component(TransformComponent::new());
        world.add_child(velocity, target).unwrap();
        world.add_child(target, input).unwrap();
        world.add_child(input, camera).unwrap();
        world
            .get_component_by_id_as_mut::<InputXRComponent>(input)
            .unwrap()
            .pose_valid = true;

        let mut eye = Transform::default();
        eye.rotation = [
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
        ];
        eye.recompute_model();
        eye.matrix_world = eye.model;
        let mut visuals = VisualWorld::default();
        visuals.set_active_xr_camera(Some(camera));
        visuals.set_camera_mono_for_target_with_transform(
            CameraTarget::Xr,
            eye.model,
            eye.model,
            eye,
        );
        let mut system = VelocitySystem::default();
        system
            .translate(&mut world, &visuals, velocity, [0.0, 0.0, -0.25], false)
            .unwrap();
        let state = world
            .get_component_by_id_as::<VelocityComponent>(velocity)
            .unwrap();
        assert!((state.linear_local_mps[0] + 0.25).abs() < 1e-5);
        assert!(state.linear_local_mps[1].abs() < 1e-5);
        assert!(state.linear_local_mps[2].abs() < 1e-5);

        // Pitch changes eye orientation but must not add vertical thrust.
        world
            .get_component_by_id_as_mut::<VelocityComponent>(velocity)
            .unwrap()
            .zero_linear();
        eye.rotation = math::quat_from_axis_angle([1.0, 0.0, 0.0], std::f32::consts::FRAC_PI_4);
        eye.recompute_model();
        eye.matrix_world = eye.model;
        visuals.set_camera_mono_for_target_with_transform(
            CameraTarget::Xr,
            eye.model,
            eye.model,
            eye,
        );
        system
            .translate(&mut world, &visuals, velocity, [0.0, 0.0, -0.25], false)
            .unwrap();
        let state = world
            .get_component_by_id_as::<VelocityComponent>(velocity)
            .unwrap();
        assert!(state.linear_local_mps[0].abs() < 1e-5);
        assert!(state.linear_local_mps[1].abs() < 1e-5);
        assert!((state.linear_local_mps[2] + 0.25).abs() < 1e-5);
    }

    #[test]
    fn parent_rotation_turns_existing_local_velocity() {
        let mut world = World::default();
        let parent = world.add_component(TransformComponent::new());
        let velocity = world.add_component(VelocityComponent::new());
        let target = world.add_component(TransformComponent::new());
        world.add_child(parent, velocity).unwrap();
        world.add_child(velocity, target).unwrap();
        propagate(&mut world, parent);
        let mut system = VelocitySystem::default();
        system
            .add_linear_local(
                &mut world,
                &VisualWorld::default(),
                velocity,
                [0.0, 0.0, -1.0],
            )
            .unwrap();
        let parent_transform = world
            .get_component_by_id_as_mut::<TransformComponent>(parent)
            .unwrap();
        parent_transform.transform.rotation = [
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
        ];
        parent_transform.transform.recompute_model();
        propagate(&mut world, parent);
        let before = TransformSystem::world_position(&world, target).unwrap();
        let mut queue = RxWorld::default();
        system.tick(&mut world, &mut queue, 1.0 / 60.0);
        propagate(&mut world, parent);
        let after = TransformSystem::world_position(&world, target).unwrap();
        assert!((after[0] - before[0] + 1.0 / 60.0).abs() < 1e-5);
        assert!((after[2] - before[2]).abs() < 1e-5);
        assert_eq!(
            world
                .get_component_by_id_as::<VelocityComponent>(velocity)
                .unwrap()
                .linear_local_mps,
            [0.0, 0.0, -1.0]
        );
    }

    #[test]
    fn velocity_needs_one_immediate_child_transform() {
        let mut world = World::default();
        let velocity = world.add_component(VelocityComponent::new());
        let mut system = VelocitySystem::default();
        let visuals = VisualWorld::default();
        assert!(
            system
                .add_linear_local(&mut world, &visuals, velocity, [1.0, 0.0, 0.0])
                .is_err()
        );
        let first = world.add_component(TransformComponent::new());
        let second = world.add_component(TransformComponent::new());
        world.add_child(velocity, first).unwrap();
        world.add_child(velocity, second).unwrap();
        assert!(
            system
                .add_linear_local(&mut world, &visuals, velocity, [1.0, 0.0, 0.0])
                .is_err()
        );
    }

    #[test]
    fn two_velocity_layers_with_intervening_transform_compose_once() {
        let mut world = World::default();
        let frame = world.add_component(TransformComponent::new());
        let mut outer_state = VelocityComponent::new();
        outer_state.set_linear_local([1.0, 0.0, 0.0]).unwrap();
        let outer_velocity = world.add_component(outer_state);
        let outer_target = world.add_component(TransformComponent::new());
        let mut inner_state = VelocityComponent::new();
        inner_state.set_linear_local([0.0, 0.0, -1.0]).unwrap();
        let inner_velocity = world.add_component(inner_state);
        let inner_target = world.add_component(TransformComponent::new());
        world.add_child(frame, outer_velocity).unwrap();
        world.add_child(outer_velocity, outer_target).unwrap();
        world.add_child(outer_target, inner_velocity).unwrap();
        world.add_child(inner_velocity, inner_target).unwrap();
        propagate(&mut world, frame);
        let mut system = VelocitySystem::default();
        let mut emit = RxWorld::default();
        system.tick(&mut world, &mut emit, 1.0 / 60.0);
        propagate(&mut world, frame);
        let outer = TransformSystem::world_position(&world, outer_target).unwrap();
        let inner = TransformSystem::world_position(&world, inner_target).unwrap();
        assert!((outer[0] - 1.0 / 60.0).abs() < 1e-5);
        assert!((inner[0] - 1.0 / 60.0).abs() < 1e-5);
        assert!((inner[2] + 1.0 / 60.0).abs() < 1e-5);
    }
}

impl VelocitySystem {
    /// Discover the nearest Gravity in this Velocity's provider scope.
    /// Disabled Gravity still wins, allowing an off wrapper to block inheritance.
    /// Every Velocity ancestor is a boundary, including disabled motion layers.
    /// Read the live tree each time so reparenting and removal cannot leave stale owners.
    pub fn gravity_provider(world: &World, velocity_id: ComponentId) -> Option<ComponentId> {
        world.get_component_by_id_as::<VelocityComponent>(velocity_id)?;
        let mut current = world.parent_of(velocity_id);
        while let Some(id) = current {
            if world
                .get_component_by_id_as::<VelocityComponent>(id)
                .is_some()
            {
                break;
            }
            if world
                .get_component_by_id_as::<GravityComponent>(id)
                .is_some()
            {
                return Some(id);
            }
            current = world.parent_of(id);
        }
        None
    }

    pub(crate) fn driven_transform(
        world: &World,
        velocity_id: ComponentId,
    ) -> Result<ComponentId, String> {
        let mut targets = world.children_of(velocity_id).iter().copied().filter(|id| {
            world
                .get_component_by_id_as::<TransformComponent>(*id)
                .is_some()
        });
        let target = targets
            .next()
            .ok_or("Velocity: needs one immediate child Transform")?;
        if targets.next().is_some() {
            return Err("Velocity: multiple child Transforms are ambiguous".into());
        }
        Ok(target)
    }

    fn ancestor_transform(world: &World, velocity_id: ComponentId) -> Option<ComponentId> {
        let mut current = world.parent_of(velocity_id);
        while let Some(id) = current {
            if world
                .get_component_by_id_as::<TransformComponent>(id)
                .is_some()
            {
                return Some(id);
            }
            current = world.parent_of(id);
        }
        None
    }

    pub(crate) fn parent_rotation(
        world: &World,
        velocity_id: ComponentId,
    ) -> Result<[f32; 4], String> {
        Self::ancestor_transform(world, velocity_id)
            .map(|id| {
                TransformSystem::world_rotation_quat_xyzw(world, id)
                    .map_err(|_| "Velocity: parent transform rotation is invalid".to_string())
            })
            .unwrap_or(Ok([0.0, 0.0, 0.0, 1.0]))
    }

    pub fn add_linear_local(
        &mut self,
        world: &mut World,
        visuals: &VisualWorld,
        velocity_id: ComponentId,
        delta_mps: [f32; 3],
    ) -> Result<(), String> {
        self.translate(world, visuals, velocity_id, delta_mps, false)
    }

    pub fn add_linear_world(
        &mut self,
        world: &mut World,
        visuals: &VisualWorld,
        velocity_id: ComponentId,
        delta_mps: [f32; 3],
    ) -> Result<(), String> {
        self.translate(world, visuals, velocity_id, delta_mps, true)
    }

    /// Apply a one-shot velocity change. Local commands snapshot the selected
    /// orientation here, while the published XR eye is still current for the click.
    fn translate(
        &mut self,
        world: &mut World,
        visuals: &VisualWorld,
        velocity_id: ComponentId,
        delta_mps: [f32; 3],
        world_space: bool,
    ) -> Result<(), String> {
        if !delta_mps.iter().all(|v| v.is_finite()) {
            return Err("Velocity.translate: delta must be finite".into());
        }
        let component = world
            .get_component_by_id_as::<VelocityComponent>(velocity_id)
            .ok_or("Velocity.translate: target is not Velocity")?;
        if !component.enabled {
            return Err("Velocity.translate: component is disabled".into());
        }
        let basis = component.rotation_basis.clone();
        let horizontal = component.horizontal;
        let previous = component.linear_local_mps;
        Self::driven_transform(world, velocity_id)?;
        let parent_rotation = Self::parent_rotation(world, velocity_id)?;

        let mut delta_world = if world_space || delta_mps == [0.0; 3] {
            delta_mps
        } else {
            let source = basis
                .as_ref()
                .map(|reference| {
                    resolve_component_ref(world, reference, None, QueryRootMode::WorldRoot)
                        .ok_or("Velocity: rotation basis did not resolve".to_string())
                })
                .transpose()?;
            let rotation = match source {
                Some(source)
                    if world
                        .get_component_by_id_as::<InputXRComponent>(source)
                        .is_some() =>
                {
                    let input = world
                        .get_component_by_id_as::<InputXRComponent>(source)
                        .unwrap();
                    if !input.enabled || !input.pose_valid {
                        return Err("Velocity: referenced InputXR has no valid pose".into());
                    }
                    let active = visuals
                        .active_xr_camera()
                        .ok_or("Velocity: no active XR camera")?;
                    let mut current = Some(active);
                    let mut owned = false;
                    while let Some(id) = current {
                        if id == source {
                            owned = true;
                            break;
                        }
                        current = world.parent_of(id);
                    }
                    if !owned {
                        return Err("Velocity: active XR camera belongs to another InputXR".into());
                    }
                    let eye = visuals
                        .visual_camera(CameraTarget::Xr)
                        .and_then(|camera| camera.eyes.first())
                        .ok_or("Velocity: active XR eye pose is unavailable")?;
                    math::mat_to_quat(eye.transform.matrix_world)
                }
                Some(source)
                    if world
                        .get_component_by_id_as::<TransformComponent>(source)
                        .is_some() =>
                {
                    TransformSystem::world_rotation_quat_xyzw(world, source)
                        .map_err(|_| "Velocity: rotation-basis transform is invalid")?
                }
                Some(_) => {
                    return Err("Velocity: rotation basis must be InputXR or Transform".into());
                }
                None => parent_rotation,
            };
            math::quat_rotate_vec3(rotation, delta_mps)
        };

        if horizontal && !world_space && delta_mps != [0.0; 3] {
            let original_len = math::vec3_len(delta_mps);
            let flat_len =
                (delta_world[0] * delta_world[0] + delta_world[2] * delta_world[2]).sqrt();
            if !flat_len.is_finite() || flat_len < 1e-6 {
                return Err("Velocity: horizontal rotation basis is degenerate".into());
            }
            let scale = original_len / flat_len;
            delta_world = [delta_world[0] * scale, 0.0, delta_world[2] * scale];
        }
        let delta_local =
            math::quat_rotate_vec3(math::quat_conjugate(parent_rotation), delta_world);
        let next = [
            previous[0] + delta_local[0],
            previous[1] + delta_local[1],
            previous[2] + delta_local[2],
        ];
        if !next.iter().all(|v| v.is_finite()) {
            return Err("Velocity.translate: resulting velocity is non-finite".into());
        }
        world
            .get_component_by_id_as_mut::<VelocityComponent>(velocity_id)
            .unwrap()
            .set_linear_local(next)
            .map_err(str::to_string)?;
        if delta_world[1] > 0.0 {
            world
                .get_component_by_id_as_mut::<VelocityComponent>(velocity_id)
                .unwrap()
                .grounded = false;
        }
        Ok(())
    }

    /// Standalone integration. Runtime scheduling uses `take_steps` and
    /// `step` so propagation and contact can commit between fixed substeps.
    pub fn tick(&mut self, world: &mut World, emit: &mut dyn SignalEmitter, dt_sec: f32) {
        for _ in 0..self.take_steps(dt_sec) {
            self.step(world, emit);
        }
    }

    pub(crate) fn take_steps(&mut self, dt_sec: f32) -> usize {
        if !dt_sec.is_finite() || dt_sec < 0.0 {
            return 0;
        }
        self.accumulator_sec += dt_sec as f64;
        let steps = ((self.accumulator_sec / STEP_SEC).floor() as usize).min(MAX_STEPS);
        self.accumulator_sec -= steps as f64 * STEP_SEC;
        if self.accumulator_sec >= STEP_SEC {
            self.dropped_since_report_sec += self.accumulator_sec;
            if self.dropped_since_report_sec >= 1.0 {
                eprintln!(
                    "[velocity_system] dropped {:.3} s of accumulated time since last report",
                    self.dropped_since_report_sec
                );
                self.dropped_since_report_sec = 0.0;
            }
            self.accumulator_sec = 0.0;
        }
        steps
    }

    pub(crate) fn step(&mut self, world: &mut World, emit: &mut dyn SignalEmitter) {
        let elapsed = STEP_SEC as f32;
        let ids: Vec<_> = world
            .all_components()
            .filter(|id| {
                world
                    .get_component_by_id_as::<VelocityComponent>(*id)
                    .is_some()
            })
            .collect();
        for id in ids {
            let Some(velocity) = world.get_component_by_id_as::<VelocityComponent>(id) else {
                continue;
            };
            if !velocity.enabled {
                continue;
            }
            let mut linear = velocity.linear_local_mps;
            world
                .get_component_by_id_as_mut::<VelocityComponent>(id)
                .unwrap()
                .grounded = false;
            let target = match Self::driven_transform(world, id) {
                Ok(target) => target,
                Err(error) => {
                    if self.reported_bad_target.insert(id) {
                        eprintln!("[velocity_system] {error}: {id:?}");
                    }
                    continue;
                }
            };
            let parent_rotation = match Self::parent_rotation(world, id) {
                Ok(rotation) => rotation,
                Err(error) => {
                    if self.reported_bad_target.insert(id) {
                        eprintln!("[velocity_system] {error}: {id:?}");
                    }
                    continue;
                }
            };
            if let Some(gravity) = Self::gravity_provider(world, id)
                .and_then(|provider| world.get_component_by_id_as::<GravityComponent>(provider))
                .filter(|gravity| gravity.enabled)
            {
                let acceleration_world = [0.0, -9.81 * gravity.coefficient, 0.0];
                let acceleration_local = math::quat_rotate_vec3(
                    math::quat_conjugate(parent_rotation),
                    acceleration_world,
                );
                for axis in 0..3 {
                    linear[axis] += acceleration_local[axis] * elapsed;
                }
                if !linear.iter().all(|value| value.is_finite()) {
                    continue;
                }
            }
            if linear == [0.0; 3] {
                world
                    .get_component_by_id_as_mut::<VelocityComponent>(id)
                    .unwrap()
                    .zero_linear();
                continue;
            }
            let world_velocity = math::quat_rotate_vec3(parent_rotation, linear);
            let delta_world = [
                world_velocity[0] * elapsed,
                world_velocity[1] * elapsed,
                world_velocity[2] * elapsed,
            ];
            let parent_world = world
                .parent_of(target)
                .and_then(|parent| TransformSystem::world_model(world, parent));
            let delta_local = if let Some(parent_world) = parent_world {
                let Some(inverse) = math::mat4_inverse(parent_world) else {
                    if self.reported_bad_target.insert(id) {
                        eprintln!("[velocity_system] singular parent basis for {id:?}");
                    }
                    continue;
                };
                let v = math::mat4_mul_vec4(
                    inverse,
                    [delta_world[0], delta_world[1], delta_world[2], 0.0],
                );
                [v[0], v[1], v[2]]
            } else {
                delta_world
            };
            if !delta_local.iter().all(|v| v.is_finite()) {
                continue;
            }
            world
                .get_component_by_id_as_mut::<VelocityComponent>(id)
                .unwrap()
                .set_linear_local(linear)
                .expect("validated velocity");
            let Some(transform) = world.get_component_by_id_as_mut::<TransformComponent>(target)
            else {
                continue;
            };
            for (position, delta) in transform.transform.translation.iter_mut().zip(delta_local) {
                *position += delta;
            }
            transform.transform.recompute_model();
            let current = transform.transform;
            emit.push_intent_now(
                target,
                IntentValue::UpdateTransform {
                    component_id: target,
                    translation: current.translation,
                    rotation_quat_xyzw: current.rotation,
                    scale: current.scale,
                },
            );
        }
    }
}
