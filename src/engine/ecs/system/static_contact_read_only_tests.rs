use super::*;
use crate::engine::ecs::component::CollisionResponse;

#[test]
fn read_only_contacts_do_not_move_or_apply_friction_with_or_without_target() {
    for has_target in [true, false] {
        let mut world = World::default();
        let (_, floor) = add_zone(
            &mut world,
            [0., -0.05, 0.],
            CollisionShape::cube_half_extents([5., 0.05, 5.]),
            CollidableMode::Static,
        );
        let floor_collider = world.children_of(floor)[0];
        world
            .get_component_by_id_as_mut::<CollidableComponent>(floor_collider)
            .unwrap()
            .friction = 1.;
        let (_, wall) = add_zone(
            &mut world,
            [0.1, 0.8, 0.],
            CollisionShape::cube_half_extents([0.1, 2., 1.]),
            CollidableMode::Static,
        );
        let (actor, zone) = add_zone(
            &mut world,
            [0., 0.8, 0.],
            CollisionShape::capsule_y(0.2, 0.8),
            CollidableMode::Slide,
        );
        let collider = world.children_of(zone)[0];
        let mut velocity = VelocityComponent::new();
        velocity.linear_local_mps = [2., -3., 1.];
        let velocity = world.add_component(velocity);
        world.add_child(velocity, actor).unwrap();
        let c = world
            .get_component_by_id_as_mut::<CollidableComponent>(collider)
            .unwrap();
        c.response = CollisionResponse::ReadOnly;
        c.movement_target_required = true;
        c.movement_target_id = has_target.then_some(actor);
        let before = world
            .get_component_by_id_as::<TransformComponent>(actor)
            .unwrap()
            .trs();
        let mut system = StaticContactSystem::default();
        assert!(
            system
                .tick_substep(
                    &mut world,
                    &crate::engine::ecs::system::AttachmentSystem::default()
                )
                .is_empty()
        );
        assert_eq!(
            world
                .get_component_by_id_as::<TransformComponent>(actor)
                .unwrap()
                .trs(),
            before
        );
        let v = world
            .get_component_by_id_as::<VelocityComponent>(velocity)
            .unwrap();
        assert_eq!(v.linear_local_mps, [2., -3., 1.]);
        assert!(!v.grounded);
        assert!(system.contacts().iter().any(|hit| hit.other_zone == wall));
        let hit = system
            .contacts()
            .iter()
            .find(|hit| hit.collidable == collider && hit.other_zone == floor)
            .unwrap();
        assert_eq!(hit.zone, zone);
        assert_eq!(hit.other, floor_collider);
        assert!(hit.separation[1] > 0.);
        // A fast observed sweep crossing the floor must be detected without
        // clamping its end pose or projecting its velocity.
        for height in [4., -4.] {
            let t = &mut world
                .get_component_by_id_as_mut::<TransformComponent>(actor)
                .unwrap()
                .transform;
            t.translation[1] = height;
            t.recompute_model();
            propagate(&mut world, actor);
            assert!(system.tick(&mut world).is_empty());
            assert_eq!(
                world
                    .get_component_by_id_as::<TransformComponent>(actor)
                    .unwrap()
                    .translation()[1],
                height
            );
        }
        assert!(system.contacts().iter().any(|hit| hit.other_zone == floor));
        assert_eq!(
            world
                .get_component_by_id_as::<VelocityComponent>(velocity)
                .unwrap()
                .linear_local_mps,
            [2., -3., 1.]
        );
        let t = &mut world
            .get_component_by_id_as_mut::<TransformComponent>(actor)
            .unwrap()
            .transform;
        t.translation[1] = 0.8;
        t.recompute_model();
        propagate(&mut world, actor);
        // Switching response uses existing geometry but now permits correction.
        let c = world
            .get_component_by_id_as_mut::<CollidableComponent>(collider)
            .unwrap();
        c.response = CollisionResponse::Slide;
        c.movement_target_id = Some(actor);
        assert!(
            !system
                .tick_substep(
                    &mut world,
                    &crate::engine::ecs::system::AttachmentSystem::default()
                )
                .is_empty()
        );
        assert!(
            world
                .get_component_by_id_as::<TransformComponent>(actor)
                .unwrap()
                .translation()[1]
                > before.translation[1]
        );
        assert!(
            world
                .get_component_by_id_as::<VelocityComponent>(velocity)
                .unwrap()
                .grounded
        );
    }
}

#[test]
fn read_only_moving_proxy_is_queryable_refitted_and_not_walkable() {
    let mut world = World::default();
    let (remote, remote_zone) = add_zone(
        &mut world,
        [0.; 3],
        CollisionShape::sphere_radius(0.5),
        CollidableMode::Slide,
    );
    let remote_collider = world.children_of(remote_zone)[0];
    world
        .get_component_by_id_as_mut::<CollidableComponent>(remote_collider)
        .unwrap()
        .response = CollisionResponse::ReadOnly;
    let (local, _) = add_zone(
        &mut world,
        [0.8, 0., 0.],
        CollisionShape::sphere_radius(0.5),
        CollidableMode::Slide,
    );
    let mut system = StaticContactSystem::default();
    let before = world
        .get_component_by_id_as::<TransformComponent>(remote)
        .unwrap()
        .trs();
    let changed = system.tick(&mut world);
    assert!(changed.contains(&local) && !changed.contains(&remote));
    assert_eq!(
        world
            .get_component_by_id_as::<TransformComponent>(remote)
            .unwrap()
            .trs(),
        before
    );
    assert!(
        system
            .contacts()
            .iter()
            .any(|contact| contact.collidable == remote_collider)
    );
    let index = system.support_index(&world);
    let surface = (0..index.len())
        .map(|i| index.surface(i))
        .find(|s| s.collidable == remote_collider)
        .unwrap();
    assert!(
        surface
            .flat_support([0.; 3], [0., 2., 0.], 0., 1.)
            .is_none()
    );
    let rebuilds = system.broad_phase.rebuilds;
    let t = &mut world
        .get_component_by_id_as_mut::<TransformComponent>(remote)
        .unwrap()
        .transform;
    t.translation[0] = 10.;
    t.recompute_model();
    propagate(&mut world, remote);
    system.tick(&mut world);
    assert_eq!(
        system.broad_phase.rebuilds, rebuilds,
        "moving a retained proxy should refit, not rebuild"
    );
    assert!(
        system.contacts().is_empty(),
        "old proxy location must be gone and self contacts excluded"
    );
    // Removing participation really does remove it from observation/indexing.
    world
        .get_component_by_id_as_mut::<CollidableComponent>(remote_collider)
        .unwrap()
        .enabled = false;
    system.tick(&mut world);
    assert!(system.contacts().is_empty());
}

#[test]
fn read_only_query_owner_excludes_body_parts_without_a_response_target() {
    let mut world = World::default();
    let owner = world.add_component(TransformComponent::new());
    let (actor, zone) = add_zone(
        &mut world,
        [0.; 3],
        CollisionShape::sphere_radius(0.5),
        CollidableMode::Slide,
    );
    world.add_child(owner, actor).unwrap();
    let collider = world.children_of(zone)[0];
    let c = world
        .get_component_by_id_as_mut::<CollidableComponent>(collider)
        .unwrap();
    c.response = CollisionResponse::ReadOnly;
    c.query_owner = Some(owner);
    c.movement_target_required = true;
    let (own_part, own_zone) = add_zone(
        &mut world,
        [0.; 3],
        CollisionShape::cube_half_extents([0.4; 3]),
        CollidableMode::Static,
    );
    world.add_child(owner, own_part).unwrap();
    let (_, floor) = add_zone(
        &mut world,
        [0., -0.6, 0.],
        CollisionShape::cube_half_extents([1., 0.2, 1.]),
        CollidableMode::Static,
    );
    let mut system = StaticContactSystem::default();
    assert!(system.tick(&mut world).is_empty());
    assert!(system.contacts().iter().any(|hit| hit.other_zone == floor));
    assert!(
        system
            .contacts()
            .iter()
            .all(|hit| hit.other_zone != own_zone)
    );
}
