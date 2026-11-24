use super::*;
use crate::engine::ecs::CommandQueue;
use crate::engine::ecs::component::{
    CollisionShape, ComponentRef, MeshComponent, RenderableComponent,
};
use crate::engine::ecs::system::TransformOperatorSystem;
use crate::engine::graphics::mesh::MeshFactory;

fn attach(world: &mut World, parent: ComponentId, child: ComponentId) {
    world.set_parent(child, Some(parent)).unwrap();
}

#[derive(Default)]
struct CapsuleEvents(Vec<(ComponentId, crate::engine::ecs::EventSignal)>);
impl SignalEmitter for CapsuleEvents {
    fn push_event(&mut self, scope: ComponentId, event: crate::engine::ecs::EventSignal) {
        self.0.push((scope, event));
    }
    fn push_intent(&mut self, _: ComponentId, _: crate::engine::ecs::IntentSignal) {}
}

struct XrCapsuleFixture {
    world: World,
    systems: super::super::super::SystemWorld,
    visuals: crate::engine::graphics::VisualWorld,
    assets: RenderAssets,
    queue: CommandQueue,
    root: ComponentId,
    frame: ComponentId,
    input: ComponentId,
    head: ComponentId,
    avatar: ComponentId,
    model: ComponentId,
}

impl XrCapsuleFixture {
    fn new(starting_head_y: f32, scale: f32) -> Self {
        Self::new_with_response(starting_head_y, scale, CollisionResponse::Slide, true)
    }

    fn new_with_response(
        starting_head_y: f32,
        scale: f32,
        response: CollisionResponse,
        has_target: bool,
    ) -> Self {
        let mut world = World::default();
        let root = world.add_component(
            TransformComponent::new()
                .with_position(0.0, 3.0, 0.0)
                .with_scale(scale, scale, scale),
        );
        let frame = world.add_component(TransformComponent::new().with_position(0.0, 0.2, 0.0));
        let input = world.add_component(InputXRComponent::on());
        world
            .get_component_by_id_as_mut::<InputXRComponent>(input)
            .unwrap()
            .pose_valid = true;
        let head =
            world.add_component(TransformComponent::new().with_position(0.0, starting_head_y, 0.0));
        let mut avc = AvatarControlComponent::new();
        avc.collision_response = response;
        avc.movement_target = Some(if has_target {
            ComponentRef::Guid(world.get_component_record(root).unwrap().guid)
        } else {
            ComponentRef::Query("[name='missing_response_target']".into())
        });
        avc.head_mount = Some(head); // Stand in for the completed humanoid splice.
        avc.model_root_local_y = -1.6;
        let avatar = world.add_component(avc);
        let model = world.add_component(TransformComponent::new().with_position(0.0, -1.6, 0.0));
        world
            .get_component_by_id_as_mut::<AvatarControlComponent>(avatar)
            .unwrap()
            .model_root_id = Some(model);
        let body = world.add_component(
            TransformComponent::new()
                .with_position(0.0, 1.0, 0.0)
                .with_scale(0.4, 2.0, 0.4),
        );
        let mesh = world.add_component(RenderableComponent::cube());
        for (parent, child) in [
            (root, frame),
            (frame, input),
            (input, head),
            (head, avatar),
            (avatar, model),
            (model, body),
            (body, mesh),
        ] {
            attach(&mut world, parent, child);
        }
        let floor = world.add_component(TransformComponent::new().with_position(
            0.0,
            3.0 + 0.2 * scale - 0.5,
            0.0,
        ));
        let zone = world.add_component(ZoneComponent::cube([10.0, 0.5, 10.0]));
        let contact = world.add_component(CollidableComponent::static_());
        attach(&mut world, floor, zone);
        attach(&mut world, zone, contact);
        let mut systems = super::super::super::SystemWorld::default();
        systems.avatar_control.register(avatar);
        let mut fixture = Self {
            world,
            systems,
            visuals: Default::default(),
            assets: RenderAssets::new(),
            queue: CommandQueue::new(),
            root,
            frame,
            input,
            head,
            avatar,
            model,
        };
        fixture
            .systems
            .transform_changed(&mut fixture.world, &mut fixture.visuals, root);
        fixture
            .systems
            .transform_changed(&mut fixture.world, &mut fixture.visuals, floor);
        for _ in 0..3 {
            fixture.tick();
        }
        assert!(
            fixture.avatar().capsule_ready,
            "fixture should settle before exercising contact"
        );
        fixture
    }

    fn tick(&mut self) {
        // Exercise the real pose-only/fixed-step/AVC frame phases headlessly.
        self.systems.tick(
            &mut self.world,
            &mut self.visuals,
            &mut self.assets,
            &InputState::default(),
            &mut self.queue,
            1.0 / 60.0,
        );
    }

    fn set_position(&mut self, id: ComponentId, position: [f32; 3]) {
        self.world
            .get_component_by_id_as_mut::<TransformComponent>(id)
            .unwrap()
            .set_position(&mut self.queue, position[0], position[1], position[2]);
        self.systems.process_commands(
            &mut self.world,
            &mut self.visuals,
            &mut self.assets,
            &mut self.queue,
        );
    }

    fn avatar(&self) -> &AvatarControlComponent {
        self.world
            .get_component_by_id_as::<AvatarControlComponent>(self.avatar)
            .unwrap()
    }

    fn position(&self, id: ComponentId) -> [f32; 3] {
        super::super::super::TransformSystem::world_position(&self.world, id).unwrap()
    }
}

fn assert_position(actual: [f32; 3], expected: [f32; 3]) {
    for axis in 0..3 {
        assert!(
            (actual[axis] - expected[axis]).abs() < 1e-4,
            "actual={actual:?} expected={expected:?}"
        );
    }
}

#[test]
fn read_only_avatar_capsule_observes_without_target_and_switches_back_to_slide() {
    let mut f = XrCapsuleFixture::new_with_response(0.8, 1.0, CollisionResponse::ReadOnly, false);
    let collider = f.avatar().capsule_collidable_id.unwrap();
    f.set_position(f.root, [0., 2.2, 0.]);
    for _ in 0..4 {
        f.tick();
    }
    assert_position(f.position(f.root), [0., 2.2, 0.]);
    assert_eq!(f.avatar().capsule_collidable_id, Some(collider));
    assert!(f.avatar().capsule_ready);
    assert!(
        f.world
            .get_component_by_id_as::<CollidableComponent>(collider)
            .unwrap()
            .enabled
    );
    assert!(
        f.systems
            .static_contact
            .contacts()
            .iter()
            .any(|contact| contact.collidable == collider)
    );
    let target = ComponentRef::Guid(f.world.get_component_record(f.root).unwrap().guid);
    let avatar = f
        .world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap();
    avatar.collision_response = CollisionResponse::Slide;
    // Without a valid target, correction must fail closed.
    f.tick();
    assert_position(f.position(f.root), [0., 2.2, 0.]);
    f.world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap()
        .movement_target = Some(target);
    for _ in 0..4 {
        f.tick();
    }
    assert_position(f.position(f.root), [0., 3.0, 0.]);
    assert_eq!(f.avatar().capsule_collidable_id, Some(collider));
    // A new read-only permission must take effect before the first contact
    // pass, not one frame after the old slide response has moved the root.
    f.world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap()
        .collision_response = CollisionResponse::ReadOnly;
    f.set_position(f.root, [0., 2.2, 0.]);
    f.tick();
    assert_position(f.position(f.root), [0., 2.2, 0.]);
}

#[test]
fn xr_capsule_keeps_physical_head_y_free_through_real_contact_frame_phases() {
    let mut f = XrCapsuleFixture::new(0.8, 1.0); // Crouched startup must not calibrate a crouched capsule.
    let capsule = f.avatar().capsule_transform_id.unwrap();
    assert_eq!(f.avatar().capsule_vertical_source, Some(f.frame));
    assert_eq!(
        f.world
            .get_component_by_id_as::<CollidableComponent>(
                f.avatar().capsule_collidable_id.unwrap()
            )
            .unwrap()
            .movement_target_id,
        Some(f.root)
    );
    for _ in 0..5 {
        for height in [0.5, 0.8, 1.6, 2.1, 1.6] {
            f.set_position(f.head, [0.1, height, -0.2]);
            f.tick();
            assert_position(f.position(capsule), [0.1, 4.2, -0.2]);
            assert_position(f.position(f.root), [0.0, 3.0, 0.0]);
            assert_position(f.position(f.head), [0.1, 3.2 + height, -0.2]);
            assert!(f.avatar().capsule_ready);
        }
    }
    // The inner gamepad frame contributes to Y even though contact corrects the outer physics root.
    f.set_position(f.frame, [0.3, 0.6, 0.4]);
    f.tick();
    assert_position(f.position(capsule), [0.4, 4.6, 0.2]);
    // Genuine simulated penetration still raises the physics root; tracking remains relative to it.
    f.set_position(f.root, [0.0, 2.2, 0.0]);
    f.tick();
    assert_position(f.position(f.root), [0.0, 2.6, 0.0]);
    assert_position(f.position(capsule), [0.4, 4.2, 0.2]);
    assert_position(f.position(f.head), [0.4, 4.8, 0.2]);
}

#[test]
fn xr_capsule_uses_uniform_world_scale_once_and_disables_unsupported_scale() {
    let mut f = XrCapsuleFixture::new(0.8, 2.0);
    let capsule = f.avatar().capsule_transform_id.unwrap();
    assert_position(f.position(capsule), [0.0, 5.4, 0.0]);
    let collider = f.avatar().capsule_collidable_id.unwrap();
    let zone = f.world.parent_of(collider).unwrap();
    assert_eq!(
        f.world
            .get_component_by_id_as::<ZoneComponent>(zone)
            .unwrap()
            .shape,
        CollisionShape::capsule_y(0.28, 1.72)
    );
    let root = f
        .world
        .get_component_by_id_as_mut::<TransformComponent>(f.root)
        .unwrap();
    *root = (*root).with_scale(2.0, 3.0, 2.0);
    f.systems
        .transform_changed(&mut f.world, &mut f.visuals, f.root);
    f.tick();
    assert!(!f.avatar().capsule_ready);
    assert!(
        !f.world
            .get_component_by_id_as::<CollidableComponent>(collider)
            .unwrap()
            .enabled
    );
}

#[test]
fn xr_capsule_still_resolves_walls_and_jump_gravity_support() {
    use crate::engine::ecs::component::{GravityComponent, VelocityComponent};
    let mut f = XrCapsuleFixture::new(1.6, 1.0);
    let gravity = f.world.add_component(GravityComponent::new());
    let velocity = f.world.add_component(VelocityComponent::new());
    attach(&mut f.world, gravity, velocity);
    attach(&mut f.world, velocity, f.root);
    f.world
        .get_component_by_id_as_mut::<VelocityComponent>(velocity)
        .unwrap()
        .set_linear_local([0.0, 4.0, 0.0])
        .unwrap();
    let mut peak = 3.0f32;
    for frame in 0..120 {
        f.set_position(f.head, [0.0, if frame % 2 == 0 { 0.4 } else { 2.1 }, 0.0]);
        f.tick();
        peak = peak.max(f.position(f.root)[1]);
    }
    assert!(peak > 3.4, "jump must still move the simulated frame");
    assert_position(f.position(f.root), [0.0, 3.0, 0.0]);
    let speed = f
        .world
        .get_component_by_id_as::<VelocityComponent>(velocity)
        .unwrap();
    assert!(speed.grounded);
    assert!(speed.linear_local_mps[1].abs() < 1e-5);
    let wall = f
        .world
        .add_component(TransformComponent::new().with_position(1.2, 4.2, 0.0));
    let zone = f.world.add_component(ZoneComponent::cube([0.1, 2.0, 2.0]));
    let collider = f.world.add_component(CollidableComponent::static_());
    attach(&mut f.world, wall, zone);
    attach(&mut f.world, zone, collider);
    f.systems
        .transform_changed(&mut f.world, &mut f.visuals, wall);
    f.set_position(f.head, [1.2, 0.4, 0.0]);
    f.tick();
    assert!(
        f.position(f.root)[0].abs() > 0.05,
        "physical X still participates in wall contact"
    );
    assert!((f.position(f.root)[1] - 3.0).abs() < 1e-4);
    assert!((f.position(f.head)[1] - 3.6).abs() < 1e-4);
}

#[test]
fn xr_capsule_mount_and_dismount_preserve_free_head_y_and_contact_exclusion() {
    use crate::engine::ecs::component::{
        InputXRGamepadComponent, MountableComponent, PointerComponent, RiderComponent,
    };
    let mut f = XrCapsuleFixture::new(1.6, 1.0);
    let reference =
        |world: &World, id| ComponentRef::Guid(world.get_component_record(id).unwrap().guid);
    let gamepad = f.world.add_component(InputXRGamepadComponent::new());
    attach(&mut f.world, f.root, gamepad);
    let rider = f.world.add_component(
        RiderComponent::new()
            .anchor(reference(&f.world, f.frame))
            .movement_root(reference(&f.world, f.root))
            .input(reference(&f.world, gamepad)),
    );
    attach(&mut f.world, f.root, rider);
    let pointer = f.world.add_component(PointerComponent::new());
    attach(&mut f.world, f.frame, pointer);
    let car = f
        .world
        .add_component(TransformComponent::new().with_position(0.0, 3.0, 0.0));
    let entry = f.world.add_component(ZoneComponent::sphere(3.0));
    let seat = f
        .world
        .add_component(TransformComponent::new().with_position(0.0, -1.0, 0.0));
    let exit = f
        .world
        .add_component(TransformComponent::new().with_position(3.0, 0.2, 0.0));
    let mountable = f.world.add_component(
        MountableComponent::new()
            .entry_zone(reference(&f.world, entry))
            .mount_anchor(reference(&f.world, seat))
            .dismount_anchor(reference(&f.world, exit)),
    );
    for child in [entry, seat, exit, mountable] {
        attach(&mut f.world, car, child);
    }
    f.systems
        .transform_changed(&mut f.world, &mut f.visuals, car);
    for _ in 0..2 {
        assert!(
            f.systems
                .attachment
                .try_mount_from_hit(&mut f.world, pointer, car, &mut f.queue)
        );
        f.systems
            .process_commands(&mut f.world, &mut f.visuals, &mut f.assets, &mut f.queue);
        f.tick();
        let mounted_y = f.position(f.root)[1];
        assert!(
            mounted_y < 2.0,
            "deliberately mount below pedestrian support"
        );
        for height in [0.4, 2.1] {
            f.set_position(f.head, [0.0, height, 0.0]);
            f.tick();
            assert!(
                (f.position(f.root)[1] - mounted_y).abs() < 1e-4,
                "mounted contact remains excluded"
            );
            assert!((f.position(f.head)[1] - (mounted_y + 0.2 + height)).abs() < 1e-4);
        }
        assert!(
            f.systems
                .attachment
                .try_dismount_for_pointer(&mut f.world, pointer, &mut f.queue)
        );
        f.systems
            .process_commands(&mut f.world, &mut f.visuals, &mut f.assets, &mut f.queue);
        f.tick();
        let after_exit = f.position(f.root);
        assert!((after_exit[1] - 3.0).abs() < 1e-4);
        for height in [0.4, 2.1] {
            f.set_position(f.head, [0.0, height, 0.0]);
            f.tick();
            assert_position(f.position(f.root), after_exit);
            assert!((f.position(f.head)[1] - (3.2 + height)).abs() < 1e-4);
        }
        // Return to the entry zone for another cycle.
        f.set_position(f.root, [0.0, 3.0, 0.0]);
        f.tick();
    }
}

#[test]
fn xr_capsule_two_avatars_keep_independent_sources_and_clean_up_owned_helpers() {
    let mut f = XrCapsuleFixture::new(1.6, 1.0);
    let baseline = f.world.component_count();
    let root = f
        .world
        .add_component(TransformComponent::new().with_position(5.0, 3.0, 0.0));
    let frame = f
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.2, 0.0));
    let input = f.world.add_component(InputXRComponent::on());
    let head = f
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.8, 0.0));
    let mut avatar = AvatarControlComponent::new();
    avatar.head_mount = Some(head);
    avatar.model_root_local_y = -1.6;
    avatar.movement_target = Some(ComponentRef::Guid(
        f.world.get_component_record(root).unwrap().guid,
    ));
    let second = f.world.add_component(avatar);
    let model = f
        .world
        .add_component(TransformComponent::new().with_position(0.0, -1.6, 0.0));
    f.world
        .get_component_by_id_as_mut::<AvatarControlComponent>(second)
        .unwrap()
        .model_root_id = Some(model);
    let body = f.world.add_component(
        TransformComponent::new()
            .with_position(0.0, 1.0, 0.0)
            .with_scale(0.4, 2.0, 0.4),
    );
    let mesh = f.world.add_component(RenderableComponent::cube());
    for (parent, child) in [
        (root, frame),
        (frame, input),
        (input, head),
        (head, second),
        (second, model),
        (model, body),
        (body, mesh),
    ] {
        attach(&mut f.world, parent, child);
    }
    f.systems.avatar_control.register(second);
    f.systems
        .transform_changed(&mut f.world, &mut f.visuals, root);
    for _ in 0..3 {
        f.tick();
    }
    let second_state = f
        .world
        .get_component_by_id_as::<AvatarControlComponent>(second)
        .unwrap();
    assert!(second_state.capsule_ready);
    assert_eq!(second_state.capsule_vertical_source, Some(frame));
    let second_capsule = second_state.capsule_transform_id.unwrap();
    let first_capsule = f.avatar().capsule_transform_id.unwrap();
    f.set_position(head, [0.3, 2.1, -0.2]);
    f.set_position(root, [5.0, 4.0, 0.0]);
    f.tick();
    assert_position(f.position(second_capsule), [5.3, 5.2, -0.2]);
    assert_position(f.position(first_capsule), [0.0, 4.2, 0.0]);
    f.systems
        .remove_subtree_immediate(&mut f.world, &mut f.visuals, root);
    f.tick();
    assert_eq!(f.world.component_count(), baseline);
    assert!(f.avatar().capsule_ready);
    assert_position(f.position(first_capsule), [0.0, 4.2, 0.0]);
}

#[test]
fn xr_capsule_gates_source_loss_and_recovers_without_duplicate_helpers() {
    let mut f = XrCapsuleFixture::new(1.6, 1.0);
    let capsule = f.avatar().capsule_transform_id.unwrap();
    let collider = f.avatar().capsule_collidable_id.unwrap();
    let count = f.world.all_components().count();
    let saved = crate::scripting::component_registry::filtered_roots_to_ce_ast(&f.world, &[f.root])
        .unwrap()
        .iter()
        .map(crate::scripting::unparser::unparse_component)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !saved.contains("sample_translation"),
        "generated sample must not survive scene save: {saved}"
    );
    assert!(
        !saved.contains("fork_trs"),
        "generated capsule fork must not survive scene save: {saved}"
    );
    f.world.detach_from_parent(f.input);
    f.set_position(f.head, [0.0, 0.3, 0.0]);
    f.tick();
    assert!(!f.avatar().capsule_ready);
    assert!(
        !f.world
            .get_component_by_id_as::<CollidableComponent>(collider)
            .unwrap()
            .enabled
    );
    assert_position(f.position(f.root), [0.0, 3.0, 0.0]);
    attach(&mut f.world, f.frame, f.input);
    f.systems
        .transform_changed(&mut f.world, &mut f.visuals, f.root);
    for _ in 0..2 {
        f.tick();
    }
    assert!(f.avatar().capsule_ready);
    assert_position(f.position(capsule), [0.0, 4.2, 0.0]);
    assert_eq!(f.world.all_components().count(), count);
    f.world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap()
        .collision_enabled = false;
    f.tick();
    assert!(!f.avatar().capsule_ready);
    assert!(
        !f.world
            .get_component_by_id_as::<CollidableComponent>(collider)
            .unwrap()
            .enabled
    );
    assert!(
        f.world
            .get_component_by_id_as::<TransformComponent>(f.model)
            .is_some()
    );
}

#[test]
fn capsule_ready_event_waits_for_xr_and_a_settled_routed_capsule() {
    let mut world = World::default();
    let assets = RenderAssets::new();
    let root = world.add_component(TransformComponent::new());
    world.get_component_record_mut(root).unwrap().name = "player_root".into();
    let input = world.add_component(InputXRComponent::on());
    let driver = world.add_component(TransformComponent::new());
    let mut avatar = AvatarControlComponent::new();
    avatar.movement_target = Some(crate::engine::ecs::component::ComponentRef::Query(
        "[name='player_root']".into(),
    ));
    let avc = world.add_component(avatar);
    let model = world.add_component(TransformComponent::new());
    let mesh = world.add_component(RenderableComponent::cube());
    for (parent, child) in [
        (root, input),
        (input, driver),
        (driver, avc),
        (avc, model),
        (model, mesh),
    ] {
        attach(&mut world, parent, child);
    }
    let mut system = AvatarControlSystem::default();
    let mut operators = TransformOperatorSystem::new();
    let mut contact = super::super::super::static_contact_system::StaticContactSystem::default();
    system.register(avc);
    let mut retargeting = JointBasisRetargetingSystem::default();
    let mut maps = HumanoidBoneMapSystem::default();
    let mut events = CapsuleEvents::default();
    for _ in 0..3 {
        system.tick(
            &mut world,
            &InputState::default(),
            &assets,
            &mut retargeting,
            &mut maps,
            &mut events,
            1.0 / 60.0,
            &operators,
        );
    }
    assert!(events.0.is_empty());
    assert!(
        world
            .get_component_by_id_as::<AvatarControlComponent>(avc)
            .unwrap()
            .capsule_transform_id
            .is_none()
    );
    // Stand in for completed avatar splicing once tracking is usable.
    world
        .get_component_by_id_as_mut::<InputXRComponent>(input)
        .unwrap()
        .pose_valid = true;
    world
        .get_component_by_id_as_mut::<AvatarControlComponent>(avc)
        .unwrap()
        .head_mount = Some(driver);
    system.tick(
        &mut world,
        &InputState::default(),
        &assets,
        &mut retargeting,
        &mut maps,
        &mut events,
        1.0 / 60.0,
        &operators,
    );
    assert!(
        world
            .get_component_by_id_as::<AvatarControlComponent>(avc)
            .unwrap()
            .capsule_transform_id
            .is_some()
    );
    assert!(
        !world
            .get_component_by_id_as::<AvatarControlComponent>(avc)
            .unwrap()
            .capsule_ready
    );
    assert!(
        events.0.is_empty(),
        "creation tick must not publish readiness"
    );
    let collider = world
        .get_component_by_id_as::<AvatarControlComponent>(avc)
        .unwrap()
        .capsule_collidable_id
        .unwrap();
    let zone = world.parent_of(collider).unwrap();
    // Missing targets keep readiness pending even with geometry present.
    world
        .get_component_by_id_as_mut::<AvatarControlComponent>(avc)
        .unwrap()
        .movement_target = Some(crate::engine::ecs::component::ComponentRef::Query(
        "[name='missing']".into(),
    ));
    system.tick(
        &mut world,
        &InputState::default(),
        &assets,
        &mut retargeting,
        &mut maps,
        &mut events,
        1.0 / 60.0,
        &operators,
    );
    assert!(events.0.is_empty());
    world
        .get_component_by_id_as_mut::<AvatarControlComponent>(avc)
        .unwrap()
        .movement_target = Some(crate::engine::ecs::component::ComponentRef::Query(
        "[name='player_root']".into(),
    ));
    for _ in 0..3 {
        refresh_capsule_routing(avc, &mut world);
        let sample = world
            .get_component_by_id_as::<AvatarControlComponent>(avc)
            .unwrap()
            .capsule_sample_id
            .unwrap();
        operators.evaluate_operator(&world, sample, crate::utils::math::mat4_identity());
        system.gate_capsule_contact(&mut world, &operators, &mut contact);
        system.tick(
            &mut world,
            &InputState::default(),
            &assets,
            &mut retargeting,
            &mut maps,
            &mut events,
            1.0 / 60.0,
            &operators,
        );
    }
    assert!(
        world
            .get_component_by_id_as::<AvatarControlComponent>(avc)
            .unwrap()
            .capsule_ready
    );
    assert_eq!(
        events.0.len(),
        1,
        "stable readiness must not repeat every frame"
    );
    assert!(
        matches!(&events.0[0], (scope, crate::engine::ecs::EventSignal::DataEvent {name, payload})
        if *scope == avc && name == "CapsuleReady" && *payload == Some(zone))
    );
    world
        .get_component_by_id_as_mut::<AvatarControlComponent>(avc)
        .unwrap()
        .collision_enabled = false;
    system.tick(
        &mut world,
        &InputState::default(),
        &assets,
        &mut retargeting,
        &mut maps,
        &mut events,
        1.0 / 60.0,
        &operators,
    );
    assert!(
        !world
            .get_component_by_id_as::<AvatarControlComponent>(avc)
            .unwrap()
            .capsule_ready
    );
    assert_eq!(events.0.len(), 1);
}

#[test]
fn rest_relative_height_ignores_stale_xr_world_matrices_and_live_pose() {
    let mut world = World::default();
    let model_root = world.add_component(TransformComponent::new());
    let hips = world.add_component(
        TransformComponent::new()
            .with_position(0.0, 99.0, 0.0)
            .with_rotation_euler(0.0, 0.4, 0.0),
    );
    let hips_rest = world.add_component(BoneRestPoseComponent::new(
        [0.0, 0.9, 0.0],
        [0.0, 0.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
    ));
    let head = world.add_component(TransformComponent::new().with_position(0.0, -42.0, 0.0));
    let head_rest = world.add_component(BoneRestPoseComponent::new(
        [0.0, 0.7, 0.0],
        [0.0, 0.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
    ));
    attach(&mut world, model_root, hips);
    attach(&mut world, hips, hips_rest);
    attach(&mut world, hips, head);
    attach(&mut world, head, head_rest);

    world
        .get_component_by_id_as_mut::<TransformComponent>(model_root)
        .unwrap()
        .transform
        .matrix_world[3][1] = 1.75;
    world
        .get_component_by_id_as_mut::<TransformComponent>(head)
        .unwrap()
        .transform
        .matrix_world[3][1] = 0.0;

    let relative = rest_model_relative_to(&world, model_root, head).unwrap();
    assert!((relative[3][1] - 1.6).abs() < 1e-6);
}

#[test]
fn generated_capsule_uses_height_once_and_routes_desktop_movement() {
    let mut world = World::default();
    let assets = RenderAssets::new();
    let driven = world.add_component(TransformComponent::new());
    let avc = world.add_component(AvatarControlComponent::new());
    let model = world.add_component(TransformComponent::new());
    let wide_mesh = world.add_component(
        TransformComponent::new()
            .with_position(0.0, 0.5, 0.0)
            .with_scale(20.0, 3.0, 8.0),
    );
    let renderable = world.add_component(RenderableComponent::cube());
    attach(&mut world, driven, avc);
    attach(&mut world, avc, model);
    attach(&mut world, model, wide_mesh);
    attach(&mut world, wide_mesh, renderable);

    let mut queue = CommandQueue::new();
    try_init_or_route_capsule(avc, &mut world, &assets, &mut queue);
    try_init_or_route_capsule(avc, &mut world, &assets, &mut queue);

    let state = world
        .get_component_by_id_as::<AvatarControlComponent>(avc)
        .unwrap();
    let capsule_t = state.capsule_transform_id.unwrap();
    let collidable_id = state.capsule_collidable_id.unwrap();
    let zone = world
        .children_of(capsule_t)
        .iter()
        .copied()
        .find(|id| world.get_component_by_id_as::<ZoneComponent>(*id).is_some())
        .unwrap();
    assert_eq!(
        world
            .get_component_by_id_as::<ZoneComponent>(zone)
            .unwrap()
            .shape,
        CollisionShape::capsule_y(0.28, 1.22)
    );
    let collidable = world
        .get_component_by_id_as::<CollidableComponent>(collidable_id)
        .unwrap();
    assert_eq!(collidable.movement_target_id, Some(driven));

    let fork = world.parent_of(capsule_t).unwrap();
    let arbitrary_pose = TransformComponent::new()
        .with_position(3.0, 4.0, 5.0)
        .with_rotation_euler(0.7, -0.4, 0.9)
        .with_scale(2.0, 3.0, 4.0)
        .transform
        .model;
    let (upright, outputs) = TransformOperatorSystem::new()
        .evaluate_operator(&world, fork, arbitrary_pose)
        .unwrap();
    assert_eq!(outputs, vec![capsule_t]);
    assert_eq!(
        [upright[3][0], upright[3][1], upright[3][2]],
        [3.0, 4.0, 5.0]
    );
    assert_eq!(upright[0], [1.0, 0.0, 0.0, 0.0]);
    assert_eq!(upright[1], [0.0, 1.0, 0.0, 0.0]);
    assert_eq!(upright[2], [0.0, 0.0, 1.0, 0.0]);
}

#[test]
fn generated_desktop_capsule_follows_a_scaled_rig() {
    let mut world = World::default();
    let assets = RenderAssets::new();
    let driven = world.add_component(TransformComponent::new());
    let avc = world.add_component(AvatarControlComponent::new());
    let model = world.add_component(TransformComponent::new());
    // A tiny player's rig scale reaches the model root's world matrix.
    world
        .get_component_by_id_as_mut::<TransformComponent>(model)
        .unwrap()
        .transform
        .matrix_world = TransformComponent::new().with_scale(0.5, 0.5, 0.5).transform.model;
    let tall_mesh = world.add_component(
        TransformComponent::new()
            .with_position(0.0, 0.5, 0.0)
            .with_scale(1.0, 3.0, 1.0),
    );
    let renderable = world.add_component(RenderableComponent::cube());
    attach(&mut world, driven, avc);
    attach(&mut world, avc, model);
    attach(&mut world, model, tall_mesh);
    attach(&mut world, tall_mesh, renderable);

    let mut queue = CommandQueue::new();
    try_init_or_route_capsule(avc, &mut world, &assets, &mut queue);

    let state = world
        .get_component_by_id_as::<AvatarControlComponent>(avc)
        .unwrap();
    let capsule_t = state.capsule_transform_id.unwrap();
    let zone = world.parent_of(state.capsule_collidable_id.unwrap()).unwrap();
    // Model-space bounds span y = -1..2; at half scale that is -0.5..1.0.
    let CollisionShape::CapsuleY {
        radius,
        half_segment,
    } = world
        .get_component_by_id_as::<ZoneComponent>(zone)
        .unwrap()
        .shape
    else {
        panic!("expected an upright capsule")
    };
    assert!((radius - 0.28).abs() < 1e-5, "{radius}");
    assert!((half_segment - 0.47).abs() < 1e-5, "{half_segment}");
    let center = world
        .get_component_by_id_as::<TransformComponent>(capsule_t)
        .unwrap()
        .transform
        .translation;
    assert!((center[1] - 0.25).abs() < 1e-5, "{center:?}");
}

#[test]
fn disabled_and_delayed_fallback_behave_deterministically() {
    let assets = RenderAssets::new();
    let mut queue = CommandQueue::new();

    let mut disabled_world = World::default();
    let avc = disabled_world.add_component(AvatarControlComponent::new().with_collision_disabled());
    let model = disabled_world.add_component(TransformComponent::new());
    attach(&mut disabled_world, avc, model);
    try_init_or_route_capsule(avc, &mut disabled_world, &assets, &mut queue);
    assert!(
        disabled_world
            .get_component_by_id_as::<AvatarControlComponent>(avc)
            .unwrap()
            .capsule_transform_id
            .is_none()
    );

    let mut world = World::default();
    let avc = world.add_component(AvatarControlComponent::new().with_avatar_height(1.4));
    let model = world.add_component(TransformComponent::new());
    let gltf = world.add_component(GLTFComponent::new("missing.glb"));
    attach(&mut world, avc, model);
    attach(&mut world, model, gltf);
    try_init_or_route_capsule(avc, &mut world, &assets, &mut queue);
    assert!(
        world
            .get_component_by_id_as::<AvatarControlComponent>(avc)
            .unwrap()
            .capsule_transform_id
            .is_none()
    );
    world
        .get_component_by_id_as_mut::<GLTFComponent>(gltf)
        .unwrap()
        .spawned = true;
    try_init_or_route_capsule(avc, &mut world, &assets, &mut queue);
    assert!(
        world
            .get_component_by_id_as::<AvatarControlComponent>(avc)
            .unwrap()
            .capsule_transform_id
            .is_some()
    );
}

#[test]
fn generated_capsule_uses_imported_mesh_bounds() {
    let mut world = World::default();
    let mut assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let driven = world.add_component(TransformComponent::new());
    let avc = world.add_component(AvatarControlComponent::new());
    let model = world.add_component(TransformComponent::new());
    let tall_shape = world.add_component(TransformComponent::new().with_scale(1.0, 2.0, 1.0));
    let renderable = world.add_component(RenderableComponent::triangle());
    let mesh = world.add_component(MeshComponent::new("avatar:body:prim0"));
    attach(&mut world, driven, avc);
    attach(&mut world, avc, model);
    attach(&mut world, model, tall_shape);
    attach(&mut world, tall_shape, renderable);
    attach(&mut world, renderable, mesh);

    assets.register_imported_mesh("avatar:body:prim0", MeshFactory::cube());
    try_init_or_route_capsule(avc, &mut world, &assets, &mut queue);

    let capsule_t = world
        .get_component_by_id_as::<AvatarControlComponent>(avc)
        .unwrap()
        .capsule_transform_id
        .expect("capsule after imported mesh registration");
    let zone = world
        .children_of(capsule_t)
        .iter()
        .copied()
        .find(|id| world.get_component_by_id_as::<ZoneComponent>(*id).is_some())
        .expect("generated zone");
    assert_eq!(
        world
            .get_component_by_id_as::<ZoneComponent>(zone)
            .unwrap()
            .shape,
        CollisionShape::capsule_y(0.28, 0.72)
    );
}

#[test]
fn explicit_capsule_target_overrides_head_and_missing_target_does_not_fallback() {
    use crate::engine::ecs::component::ComponentRef;
    let mut world = World::default();
    let root = world.add_component(TransformComponent::new());
    world.get_component_record_mut(root).unwrap().name = "falling_root".into();
    let head = world.add_component(TransformComponent::new());
    let mut config = AvatarControlComponent::new();
    config.movement_target = Some(ComponentRef::Query("[name='falling_root']".into()));
    let avc = world.add_component(config);
    attach(&mut world, root, head);
    attach(&mut world, head, avc);
    assert_eq!(automatic_avc_movement_target(&world, avc), Some(root));
    world
        .get_component_by_id_as_mut::<AvatarControlComponent>(avc)
        .unwrap()
        .movement_target = Some(ComponentRef::Query("[name='missing_root']".into()));
    assert_eq!(automatic_avc_movement_target(&world, avc), None);
}

#[test]
fn xr_routes_to_transform_above_input_xr() {
    let mut world = World::default();
    let outer = world.add_component(TransformComponent::new());
    let input_xr = world.add_component(InputXRComponent::on());
    let driven = world.add_component(TransformComponent::new());
    let avc = world.add_component(AvatarControlComponent::new());
    attach(&mut world, outer, input_xr);
    attach(&mut world, input_xr, driven);
    attach(&mut world, driven, avc);
    assert_eq!(automatic_avc_movement_target(&world, avc), Some(outer));
}
