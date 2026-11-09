use super::tests::start_settings_scene;
use crate::engine::ecs::component::{TransformComponent, VelocityComponent};
use crate::engine::ecs::system::SystemWorld;
use crate::engine::ecs::system::static_contact_system::StaticContactSystem;
use crate::engine::ecs::{
    CommandQueue, ComponentId, EventSignal, IntentValue, KeyboardEvent, Signal, SignalEmitter,
    World,
};
use crate::engine::graphics::{RenderAssets, VisualWorld};
use crate::scripting::RuntimeSpecSession;

pub(super) fn wait_for_material_compilation(visuals: &VisualWorld) {
    // Short-lived scene fixtures must finish shaderc jobs before dropping their
    // material definitions or exiting the process (as in the pickup fixture).
    let definitions: Vec<_> = visuals
        .materials
        .instances()
        .filter_map(|(_, instance)| {
            visuals
                .materials
                .definition(instance.definition)?
                .custom
                .clone()
        })
        .collect();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while definitions
        .iter()
        .any(|definition| definition.compiled.get().is_none())
    {
        assert!(
            std::time::Instant::now() < deadline,
            "material compilation timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

struct Fixture {
    world: World,
    systems: SystemWorld,
    assets: RenderAssets,
    visuals: VisualWorld,
    queue: CommandQueue,
    session: RuntimeSpecSession,
}

impl Fixture {
    fn new(source: &str) -> Self {
        let mut world = World::default();
        let mut systems = SystemWorld::default();
        let mut assets = RenderAssets::new();
        let mut queue = CommandQueue::new();
        let (session, output) = start_settings_scene(
            source,
            "examples/_car_test.mms",
            &mut world,
            &mut systems.rx,
            Some(&mut assets),
            &mut queue,
        )
        .unwrap();
        let mut visuals = VisualWorld::default();
        for intent in output.intents {
            queue.push_intent_now(ComponentId::default(), intent);
        }
        systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
        wait_for_material_compilation(&visuals);
        Self {
            world,
            systems,
            assets,
            visuals,
            queue,
            session,
        }
    }
    fn named(&self, label: &str) -> ComponentId {
        self.world
            .all_components()
            .find(|&id| self.world.component_label(id) == Some(label))
            .unwrap()
    }
    fn event(&mut self, source: ComponentId, event: EventSignal) {
        self.systems
            .rx
            .dispatch_event_handlers(&mut self.world, &Signal::event(source, event));
        let output = self.session.service_callbacks(
            &mut self.world,
            &mut self.systems.rx,
            Some(&mut self.assets),
            &mut self.queue,
        );
        assert!(output.errors.is_empty(), "{:?}", output.errors);
        for intent in output.intents {
            self.queue.push_intent_now(source, intent);
        }
        self.systems.process_commands(
            &mut self.world,
            &mut self.visuals,
            &mut self.assets,
            &mut self.queue,
        );
    }
    fn click_power(&mut self) {
        let panel = self.named("pickup_power_button");
        self.event(
            panel,
            EventSignal::Click {
                raycaster: ComponentId::default(),
                renderable: panel,
                hit_point: [0.; 3],
                screen_pos_px: None,
            },
        );
    }
    fn velocity(&self) -> &VelocityComponent {
        self.world
            .get_component_by_id_as(self.named("test_car_velocity"))
            .unwrap()
    }
    fn step(&mut self) {
        self.systems
            .velocity
            .step(&mut self.world, &mut self.systems.rx);
        let body = self.named("test_car");
        self.systems
            .transform_changed(&mut self.world, &mut self.visuals, body);
    }
}

#[test]
fn xr_vehicle_input_handlers_exist_only_while_powered_and_occupied() {
    use crate::engine::ecs::SignalKind;
    use crate::engine::ecs::component::MountableComponent;
    let mut f = Fixture::new(
        r#"
        import {display_car_xr} from "../assets/components/vehicles/display_car.mms"
        display_car_xr("test_car",[0,0,0],0,"test_mount",world)
        display_car_xr("other_car",[14,0,0],0,"other_mount",world)
    "#,
    );
    let check = |f: &Fixture, active| {
        for kind in [
            SignalKind::XrButtonDown,
            SignalKind::XrButtonUp,
            SignalKind::XrAxisChanged,
            SignalKind::FrameTick,
        ] {
            assert_eq!(
                f.systems.rx.has_global_handlers(kind),
                active,
                "unexpected handler registration for {kind:?}"
            );
        }
    };
    check(&f, false);
    let body = f.named("test_car");
    let mountable = f
        .world
        .children_of(body)
        .iter()
        .copied()
        .find(|&id| {
            f.world
                .get_component_by_id_as::<MountableComponent>(id)
                .is_some()
        })
        .unwrap();
    f.click_power();
    check(&f, false); // Power alone does not register unoccupied vehicle controls.
    for _ in 0..3 {
        f.event(
            mountable,
            EventSignal::MountStarted {
                rider: ComponentId::default(),
                mountable,
            },
        );
        check(&f, true);
        f.click_power();
        check(&f, false);
        f.click_power();
        check(&f, true);
        f.event(
            mountable,
            EventSignal::MountEnded {
                rider: ComponentId::default(),
                mountable,
            },
        );
        check(&f, false);
    }
}

#[test]
fn display_car_power_gates_controls_but_keeps_physics_and_momentum() {
    let mut f = Fixture::new(
        r#"
        import {display_car_desktop} from "../assets/components/vehicles/display_car.mms"
        display_car_desktop("test_car",[0,4,0],0,"test_mount")
    "#,
    );
    let body = f.named("test_car");
    let mountable = f
        .world
        .children_of(body)
        .iter()
        .copied()
        .find(|&id| {
            f.world
                .get_component_by_id_as::<crate::engine::ecs::component::MountableComponent>(id)
                .is_some()
        })
        .unwrap();
    let key = || {
        EventSignal::KeyDown(KeyboardEvent {
            code: Some("KeyW".into()),
            key: "w".into(),
        })
    };
    f.event(
        mountable,
        EventSignal::MountStarted {
            rider: ComponentId::default(),
            mountable,
        },
    );
    f.event(ComponentId::default(), key());
    assert_eq!(f.velocity().linear_local_mps, [0.; 3], "starts powered off");
    f.step();
    assert!(
        f.velocity().linear_local_mps[1] < 0.,
        "gravity runs while off"
    );
    f.click_power();
    f.event(ComponentId::default(), key());
    assert!(
        f.velocity().linear_local_mps[1] < 0.,
        "commands preserve falling velocity"
    );
    assert_eq!(f.velocity().linear_local_mps[2], -5.);
    f.click_power();
    let before = f.velocity().linear_local_mps;
    f.event(
        ComponentId::default(),
        EventSignal::FrameTick { dt_sec: 0.1 },
    );
    assert_eq!(
        f.velocity().linear_local_mps,
        before,
        "off controls preserve momentum"
    );
    let old = f
        .world
        .get_component_by_id_as::<TransformComponent>(body)
        .unwrap()
        .translation();
    f.step();
    let new = f
        .world
        .get_component_by_id_as::<TransformComponent>(body)
        .unwrap()
        .translation();
    assert!(new[1] < old[1] && new[2] < old[2]);
    f.click_power();
    f.event(
        mountable,
        EventSignal::MountEnded {
            rider: ComponentId::default(),
            mountable,
        },
    );
    f.event(ComponentId::default(), key());
    assert_eq!(
        f.velocity().linear_local_mps[2],
        -5.,
        "unoccupied controls stay bypassed"
    );
}

#[test]
fn display_car_slides_along_a_wall_without_losing_tangent_velocity() {
    let mut f = Fixture::new(
        r##"
        import {display_car} from "../assets/components/vehicles/display_car.mms"
        T.position(6,5,0) {Zone.cube([0.1,10,50]) {Collidable.static().friction(0) {}}}
        display_car("test_car",[0,4,-3],0,"test_mount",[])
        query("#test_car_gravity").set_enabled(false)
    "##,
    );
    let motion = f.named("test_car_velocity");
    f.world
        .get_component_by_id_as_mut::<VelocityComponent>(motion)
        .unwrap()
        .linear_local_mps = [4., 0., 2.];
    let mut contact = StaticContactSystem::default();
    for _ in 0..240 {
        f.step();
        for id in contact.tick(&mut f.world) {
            f.systems
                .transform_changed(&mut f.world, &mut f.visuals, id);
        }
    }
    let body = f.named("test_car");
    let position = f
        .world
        .get_component_by_id_as::<TransformComponent>(body)
        .unwrap()
        .translation();
    assert!(
        (position[0] - 1.5).abs() < 0.001,
        "wall contact: {position:?}"
    );
    assert!(
        (position[2] - 1.).abs() < 0.001,
        "slide along wall: {position:?}"
    );
    assert_eq!(f.velocity().linear_local_mps, [0., 0., 2.]);
}

#[test]
fn every_car_example_uses_native_motion_and_slide_contact() {
    use crate::engine::ecs::component::{CollidableComponent, CollidableMode, GLTFComponent};
    for path in [
        "examples/paper-craft.mms",
        "examples/car.mms",
        "examples/mittens-corp.mms",
        "examples/mittens-corp-desktop.mms",
        "examples/mittens-corp-agc.mms",
        "examples/toon-outline-bisket.mms",
        "examples/sakura-drone-conversion.mms",
        "examples/sakura-drone-conversion-desktop.mms",
    ] {
        let source = std::fs::read_to_string(path).unwrap();
        // Preserve the actual module path when resolving scene imports.
        let mut world = World::default();
        let mut rx = crate::engine::ecs::RxWorld::default();
        let mut queue = CommandQueue::new();
        let mut assets = RenderAssets::new();
        let (_session, output) = start_settings_scene(
            &source,
            path,
            &mut world,
            &mut rx,
            Some(&mut assets),
            &mut queue,
        )
        .unwrap();
        assert!(output.errors.is_empty(), "{path}: {:?}", output.errors);
        // This is a topology audit. Apply prefab attachments without starting
        // native XR/audio services for every scene in the same test process.
        for intent in output.intents {
            if let IntentValue::Attach { parent, child } = intent {
                world.add_child(parent, child).unwrap();
            }
        }
        let models: Vec<_> = world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<GLTFComponent>(id)
                    .is_some_and(|m| m.uri == "assets/models/car.glb")
            })
            .collect();
        assert!(!models.is_empty(), "{path}");
        for model in models {
            let mut parent = world.parent_of(model);
            let mut body = None;
            while let Some(id) = parent {
                if world
                    .get_component_by_id_as::<VelocityComponent>(id)
                    .is_some()
                {
                    break;
                }
                body = Some(id);
                parent = world.parent_of(id);
            }
            assert!(parent.is_some(), "{path}: missing car velocity");
            let mut pending = vec![body.unwrap()];
            let mut slide = false;
            while let Some(id) = pending.pop() {
                slide |= world
                    .get_component_by_id_as::<CollidableComponent>(id)
                    .is_some_and(|c| c.mode == CollidableMode::Slide);
                pending.extend_from_slice(world.children_of(id));
            }
            assert!(slide, "{path}: missing car slide contact");
        }
    }
}

#[test]
fn display_car_slide_hull_settles_at_any_heading_and_falls_after_support_removal() {
    for yaw in [0., 0.7, 1.57, 3.14] {
        for height in [0.5, 5.] {
            let mut f = Fixture::new(&format!(
                r#"
                import {{display_car}} from "../assets/components/vehicles/display_car.mms"
                T.position(0,-0.1,0) {{name="floor" Zone.cube([15,0.1,15]) {{Collidable.static() {{}}}}}}
                display_car("test_car",[0,{height},0],{yaw},"test_mount",[])
            "#
            ));
            let mut contact = StaticContactSystem::default();
            for _ in 0..240 {
                f.step();
                for id in contact.tick(&mut f.world) {
                    f.systems
                        .transform_changed(&mut f.world, &mut f.visuals, id);
                }
            }
            let body = f.named("test_car");
            let y = f
                .world
                .get_component_by_id_as::<TransformComponent>(body)
                .unwrap()
                .translation()[1];
            assert!(
                (y + 0.04635).abs() < 0.001,
                "yaw {yaw}, height {height}: {y}"
            );
            assert!(f.velocity().linear_local_mps[1].abs() < 0.001);
            let floor = f.named("floor");
            f.world
                .get_component_by_id_as_mut::<TransformComponent>(floor)
                .unwrap()
                .transform
                .translation[0] = 100.;
            f.world
                .get_component_by_id_as_mut::<TransformComponent>(floor)
                .unwrap()
                .transform
                .recompute_model();
            f.systems
                .transform_changed(&mut f.world, &mut f.visuals, floor);
            for _ in 0..30 {
                f.step();
                contact.tick(&mut f.world);
            }
            assert!(f.velocity().linear_local_mps[1] < -1.);
        }
    }
}

#[test]
fn mittens_corp_desktop_routes_mounted_wasd_to_the_car() {
    use crate::engine::ecs::component::MountableComponent;
    use crate::engine::ecs::{EventSignal, KeyboardEvent, Signal};

    let mut world = World::default();
    let mut rx = crate::engine::ecs::RxWorld::default();
    let mut queue = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let (mut session, output) = start_settings_scene(
        include_str!("../../examples/mittens-corp-desktop.mms"),
        "examples/mittens-corp-desktop.mms",
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .expect("mittens-corp desktop scene should start");
    assert!(output.errors.is_empty(), "{:?}", output.errors);

    let car_root = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("left_display_car"))
        .expect("desktop scene should expose the display car");
    let mountable = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<MountableComponent>(id)
                .is_some()
        })
        .expect("desktop scene should expose a mountable car");
    let motion = world.parent_of(car_root).unwrap();
    let panel = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("pickup_power_button"))
        .unwrap();
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            panel,
            EventSignal::Click {
                raycaster: ComponentId::default(),
                renderable: panel,
                hit_point: [0.; 3],
                screen_pos_px: None,
            },
        ),
    );

    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            mountable,
            EventSignal::MountStarted {
                rider: ComponentId::default(),
                mountable,
            },
        ),
    );
    let activation = session.service_callbacks(&mut world, &mut rx, None, &mut queue);
    assert!(activation.errors.is_empty(), "{:?}", activation.errors);
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            ComponentId::default(),
            EventSignal::KeyDown(KeyboardEvent {
                code: Some("KeyW".to_string()),
                key: "w".to_string(),
            }),
        ),
    );
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            ComponentId::default(),
            EventSignal::FrameTick { dt_sec: 1.0 },
        ),
    );
    let drive_output = session.service_callbacks(&mut world, &mut rx, None, &mut queue);
    assert!(drive_output.errors.is_empty(), "{:?}", drive_output.errors);
    assert!(drive_output.intents.iter().any(|intent| matches!(
        intent, crate::engine::ecs::IntentValue::VelocityTranslate {component_id, delta_mps, ..}
        if *component_id==motion && delta_mps[2]==-5.
    )));

    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            mountable,
            EventSignal::MountEnded {
                rider: ComponentId::default(),
                mountable,
            },
        ),
    );
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            ComponentId::default(),
            EventSignal::FrameTick { dt_sec: 1.0 },
        ),
    );
    let dismount_output = session.service_callbacks(&mut world, &mut rx, None, &mut queue);
    assert!(
        dismount_output.errors.is_empty(),
        "{:?}",
        dismount_output.errors
    );
    assert!(dismount_output.intents.iter().all(|intent| !matches!(
        intent,
        crate::engine::ecs::IntentValue::VelocityTranslate { component_id, .. } if *component_id == motion
    )));
}

#[test]
fn mittens_corp_managed_car_attachments_survive_topology_cleanup() {
    use crate::engine::ecs::component::{
        InputComponent, InputXRGamepadComponent, MountableComponent, PointerComponent,
        QueryRootMode, RiderComponent, resolve_component_ref,
    };
    use crate::engine::ecs::system::{SystemWorld, TransformSystem};

    for (path, source, expected_car) in [
        (
            "examples/mittens-corp.mms",
            include_str!("../../examples/mittens-corp.mms"),
            "left_display_car",
        ),
        (
            "examples/paper-craft.mms",
            include_str!("../../examples/paper-craft.mms"),
            "paper_craft_road_car_north",
        ),
    ] {
        let mut world = World::default();
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let mut assets = RenderAssets::new();
        let mut queue = CommandQueue::new();
        let (_session, output) = start_settings_scene(
            source,
            path,
            &mut world,
            &mut systems.rx,
            Some(&mut assets),
            &mut queue,
        )
        .unwrap();
        assert!(output.errors.is_empty(), "{path}: {:?}", output.errors);
        for intent in output.intents {
            queue.push_intent_now(ComponentId::default(), intent);
        }
        systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
        wait_for_material_compilation(&visuals);

        let rider_id = world
            .all_components()
            .find(|&id| world.get_component_by_id_as::<RiderComponent>(id).is_some())
            .unwrap();
        let rider = world
            .get_component_by_id_as::<RiderComponent>(rider_id)
            .unwrap()
            .clone();
        let resolve =
            |world: &World, owner, source: &Option<crate::engine::ecs::component::ComponentRef>| {
                resolve_component_ref(
                    world,
                    source.as_ref().unwrap(),
                    Some(owner),
                    QueryRootMode::ParentScope { levels_up: 1 },
                )
                .unwrap_or_else(|| panic!("{path}: unresolved {source:?} for {owner:?}"))
            };
        let root = resolve(&world, rider_id, &rider.movement_root);
        let anchor = resolve(&world, rider_id, &rider.anchor);
        let input = resolve(&world, rider_id, &rider.input);
        let mountable_id = world
            .all_components()
            .find(|&id| {
                world
                    .get_component_by_id_as::<MountableComponent>(id)
                    .is_some()
                    && world
                        .parent_of(id)
                        .and_then(|parent| world.component_label(parent))
                        == Some(expected_car)
            })
            .unwrap();
        let mountable = world
            .get_component_by_id_as::<MountableComponent>(mountable_id)
            .unwrap()
            .clone();
        let mount_anchor = resolve(&world, mountable_id, &mountable.mount_anchor);
        let car = world.parent_of(mountable_id).unwrap();
        assert_eq!(world.component_label(car), Some(expected_car));
        let entry_frame = world.find_component(car, "#car_entry_zone_frame").unwrap();
        let pointer = world.add_component(PointerComponent::new());
        world.add_child(anchor, pointer).unwrap();
        let original_parent = world.parent_of(root);
        let baseline = world.component_count();

        for external_detach in [false, true] {
            // Move the authored rider's probe into the authored car entry frame.
            // Use real transform intents/propagation rather than fabricated mount events.
            let entry = TransformSystem::world_position(&world, entry_frame).unwrap();
            let probe = TransformSystem::world_position(&world, anchor).unwrap();
            let t = world
                .get_component_by_id_as::<TransformComponent>(root)
                .unwrap();
            let translation =
                std::array::from_fn(|axis| t.translation()[axis] + entry[axis] - probe[axis]);
            queue.push_intent_now(
                root,
                IntentValue::UpdateTransform {
                    component_id: root,
                    translation,
                    rotation_quat_xyzw: t.rotation_quat_xyzw(),
                    scale: t.scale(),
                },
            );
            systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
            assert!(
                systems
                    .attachment
                    .try_mount_from_hit(&mut world, pointer, car, &mut systems.rx),
                "{path}"
            );
            systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
            let relationship = systems
                .attachment
                .mounted_relationship(&world, rider_id)
                .unwrap();
            assert_eq!(relationship.movement_root, root);
            assert_eq!(relationship.rider_anchor, anchor);
            assert_eq!(relationship.mount_anchor, mount_anchor);
            assert_eq!(relationship.mountable, mountable_id);
            assert!(world.attachment_is_live(relationship.attachment));
            assert_eq!(world.component_count(), baseline + 2);
            if let Some(input) = world.get_component_by_id_as::<InputComponent>(input) {
                assert!(!input.translation_enabled);
                assert!(input.rotation_enabled);
            } else {
                assert!(
                    !world
                        .get_component_by_id_as::<InputXRGamepadComponent>(input)
                        .unwrap()
                        .locomotion
                );
            }

            // Moving the car must carry the rider through the managed follow target.
            let before = TransformSystem::world_position(&world, root).unwrap();
            let t = world
                .get_component_by_id_as::<TransformComponent>(car)
                .unwrap();
            let mut moved = t.translation();
            moved[0] += 1.0;
            queue.push_intent_now(
                car,
                IntentValue::UpdateTransform {
                    component_id: car,
                    translation: moved,
                    rotation_quat_xyzw: t.rotation_quat_xyzw(),
                    scale: t.scale(),
                },
            );
            systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
            let after = TransformSystem::world_position(&world, root).unwrap();
            assert!(
                (after[0] - before[0] - 1.0).abs() < 1e-4,
                "{path}: {before:?} -> {after:?}"
            );

            if external_detach {
                world.detach_from_parent(root);
                systems.attachment.tick(&mut world, &mut systems.rx);
            } else {
                systems.attachment.begin_frame();
                assert!(systems.attachment.try_dismount_for_pointer(
                    &mut world,
                    pointer,
                    &mut systems.rx
                ));
            }
            systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
            assert_eq!(
                world.parent_of(root),
                if external_detach {
                    None
                } else {
                    original_parent
                }
            );
            assert!(
                systems
                    .attachment
                    .mounted_relationship(&world, rider_id)
                    .is_none()
            );
            assert_eq!(world.component_count(), baseline);
            if let Some(input) = world.get_component_by_id_as::<InputComponent>(input) {
                assert!(input.translation_enabled);
            } else {
                assert!(
                    world
                        .get_component_by_id_as::<InputXRGamepadComponent>(input)
                        .unwrap()
                        .locomotion
                );
            }
        }
    }
}
