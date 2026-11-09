//! Scene-authored stage contacts with the same routed slide capsule used by AVC.
use crate::engine::ecs::component::{
    AvatarControlComponent, CollidableComponent, GravityComponent, InputXRComponent,
    InputXRGamepadComponent, TransformComponent, WalkingIkComponent, ZoneComponent,
};
use crate::engine::ecs::system::static_contact_system::StaticContactSystem;
use crate::engine::ecs::system::{
    CameraSystem, LightSystem, TransformOperatorSystem, TransformSystem,
};
use crate::engine::ecs::{CommandQueue, ComponentId, RxWorld, World};
use crate::engine::graphics::{RenderAssets, VisualWorld};

fn propagate(world: &mut World, id: ComponentId) {
    TransformSystem::new().transform_changed(
        world,
        &mut VisualWorld::default(),
        id,
        &mut TransformOperatorSystem::new(),
        &mut CameraSystem::new(),
        &mut LightSystem::new(),
    );
}
fn place(world: &mut World, id: ComponentId, p: [f32; 3]) {
    let t = world
        .get_component_by_id_as_mut::<TransformComponent>(id)
        .unwrap();
    t.transform.translation = p;
    t.transform.recompute_model();
    propagate(world, id);
}
fn position(world: &World, id: ComponentId) -> [f32; 3] {
    world
        .get_component_by_id_as::<TransformComponent>(id)
        .unwrap()
        .translation()
}
#[test]
fn xr_settings_walking_routes_to_motion_root_and_stage_blocks_floor_and_boxes() {
    let (mut world, mut rx, mut queue, mut assets) = (
        World::default(),
        RxWorld::default(),
        CommandQueue::new(),
        RenderAssets::new(),
    );
    let (_session, _) = super::tests::start_settings_scene(
        include_str!("../../examples/xr-settings.mms"),
        "examples/xr-settings.mms",
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .unwrap();
    let named = |name: &str| {
        world
            .all_components()
            .find(|&id| world.component_label(id) == Some(name))
            .unwrap()
    };
    let motion = named("mitten_locomotion_root");
    let avatar = named("mitten_avatar_control");
    let gravity = named("player_gravity");
    let config = world
        .get_component_by_id_as::<AvatarControlComponent>(avatar)
        .unwrap();
    assert!(config.collision_enabled && config.walking_enabled && config.crouch_enabled);
    assert!(world.children_of(avatar).iter().any(|&id| {
        world
            .get_component_by_id_as::<WalkingIkComponent>(id)
            .is_some_and(|w| w.enabled)
    }));
    assert!(
        !world
            .get_component_by_id_as::<GravityComponent>(gravity)
            .unwrap()
            .enabled,
        "gravity waits for CapsuleReady"
    );
    let input = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<InputXRComponent>(id)
                .is_some()
        })
        .unwrap();
    assert_eq!(
        crate::engine::ecs::system::input_xr_gamepad_system::xr_locomotion_target_transform(
            &world, input
        ),
        Some(motion)
    );
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<InputXRGamepadComponent>(id)
            .is_some_and(|g| g.locomotion && !g.enable_flying)
    }));

    // Loading an XR runtime/model is independent of contact response. Use a
    // representative upright capsule on the real movement target and authored
    // stage colliders, then exercise the production contact solver headlessly.
    for id in world.world_roots() {
        propagate(&mut world, id);
    }
    let frame = world.add_component(TransformComponent::new().with_position(0., 0.8, 0.));
    let zone = world.add_component(ZoneComponent::capsule_y(0.25, 0.55));
    let collider = world
        .add_component(CollidableComponent::slide().with_runtime_movement_target(Some(motion)));
    world.add_child(motion, frame).unwrap();
    world.add_child(frame, zone).unwrap();
    world.add_child(zone, collider).unwrap();
    let mut contacts = StaticContactSystem::default();
    place(&mut world, motion, [0., 0.2, 0.]);
    for _ in 0..30 {
        let mut p = position(&world, motion);
        p[1] -= 0.025;
        place(&mut world, motion, p);
        contacts.tick(&mut world);
        propagate(&mut world, motion);
    }
    assert!(
        position(&world, motion)[1].abs() < 0.001,
        "the floor must support the feet"
    );
    contacts.forget_zone(zone);
    place(&mut world, motion, [0., 0., -2.8]);
    contacts.tick(&mut world);
    for _ in 0..70 {
        let mut p = position(&world, motion);
        p[0] += 0.03;
        place(&mut world, motion, p);
        contacts.tick(&mut world);
        propagate(&mut world, motion);
    }
    let p = position(&world, motion);
    assert!(
        (p[0] - 1.0).abs() < 0.002,
        "pile_b_base_left must block the capsule at its front edge: {p:?}"
    );
    contacts.forget_zone(zone);
    place(&mut world, motion, [1.45, 1.1, -2.8]);
    for _ in 0..30 {
        let mut p = position(&world, motion);
        p[1] -= 0.025;
        place(&mut world, motion, p);
        contacts.tick(&mut world);
        propagate(&mut world, motion);
    }
    assert!(
        (position(&world, motion)[1] - 0.8).abs() < 0.002,
        "the cube top must support the feet"
    );
}
