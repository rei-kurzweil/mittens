use mittens_engine::engine::{
    Universe,
    ecs::{ComponentId, EventSignal, SignalEmitter, World, component::*, system::TransformSystem},
    graphics::{CpuMesh, MeshHandle, MeshUploader, TextureHandle, TextureUploader},
};

#[derive(Default)]
struct Upload(u32);
impl MeshUploader for Upload {
    fn upload_mesh(&mut self, _: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(MeshHandle(self.0))
    }
}
impl TextureUploader for Upload {
    fn upload_texture_rgba8(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(TextureHandle(self.0))
    }
    fn upload_texture_bc7(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
        _: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(TextureHandle(self.0))
    }
}

fn named(world: &World, label: &str) -> ComponentId {
    world
        .all_components()
        .find(|&id| world.component_label(id) == Some(label))
        .unwrap_or_else(|| panic!("missing {label}"))
}
fn tick(scene: &mut Universe, count: usize) {
    for _ in 0..count {
        scene.update(0.05, &Default::default());
    }
}
fn click(scene: &mut Universe, toggle: ComponentId) {
    scene.command_queue.push_event(
        toggle,
        EventSignal::Click {
            raycaster: ComponentId::default(),
            renderable: toggle,
            hit_point: [0.0; 3],
            screen_pos_px: None,
        },
    );
    tick(scene, 4);
}
fn key(scene: &mut Universe, root: ComponentId, code: &str, down: bool) {
    let event = mittens_engine::engine::ecs::KeyboardEvent {
        code: Some(code.into()),
        key: code.into(),
    };
    scene.command_queue.push_event(
        root,
        if down {
            EventSignal::KeyDown(event)
        } else {
            EventSignal::KeyUp(event)
        },
    );
    tick(scene, 2);
}
#[test]
fn stores_rebuild_and_pickups_mount_drive_brake_and_dismount() {
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(r##"
        import { ship_input_controller } from "../assets/components/ship_input_controller.mms"
        ship_input_controller(false)
        import { ship_store_accordion } from "../assets/components/ship_store_accordion.mms"
        import { pickup_truck } from "../assets/components/vehicles/pickup_truck.mms"
        import { ship_convenience_store } from "../assets/components/rooms/ship_convenience_store.mms"
        import { ship_solid } from "../assets/components/ship_parts.mms"
        fn fixture() { return T { name="store_fixture" ship_solid([0,1,2],[1,1,1],[0.4,0.5,0.4]) PL.intensity(3) } }
        T.position(10,0,0) { ship_store_accordion("test_store","TEST STORE",fixture,5,3,null) }
        T.position(20,0,0) { ship_store_accordion("retail_test","CONVENIENCE",ship_convenience_store,12,6,null) }
        pickup_truck(null,null,null,true)
        ship_solid([0,-0.1,0],[60,0.2,60],[0.3,0.3,0.3])
        T.position(-1.5,0,0) {
            name="test_rider_root"
            Rider.anchor("#test_rider_eye").movement_root("#test_rider_root").input("#test_rider_input") {}
            I.speed(2) { name="test_rider_input"
                T.position(0,1.65,0) { name="test_rider_eye" Pointer {name="test_rider_pointer"} }
            }
        }
    "##, "examples/_ship_lifecycle_test.mms").unwrap();
    let definitions: Vec<_> = scene
        .world
        .all_components()
        .filter_map(|id| {
            let registry = &scene.visuals.materials;
            registry
                .definition(registry.source_definition(id)?)?
                .custom
                .clone()
        })
        .collect();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while definitions.iter().any(|d| d.compiled.get().is_none()) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    tick(&mut scene, 2);

    // The replacement GLB has to load before its body can be hit for boarding.
    let mut upload = Upload::default();
    for _ in 0..3 {
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut upload,
            &mut scene.command_queue,
        );
        tick(&mut scene, 1);
    }

    let store = named(&scene.world, "test_store_accordion");
    let toggle = scene
        .world
        .find_component(store, "#accordion_toggle")
        .unwrap();
    for _ in 0..3 {
        let original = named(&scene.world, "store_fixture");
        click(&mut scene, toggle);
        assert!(
            !scene
                .world
                .all_components()
                .any(|id| scene.world.component_label(id) == Some("store_fixture")),
            "minimize must unload the store"
        );
        assert!(
            scene
                .world
                .find_component(store, "#accordion_body")
                .is_none()
        );
        click(&mut scene, toggle);
        let fresh = named(&scene.world, "store_fixture");
        assert_ne!(fresh, original, "restore must build a fresh subtree");
        assert_eq!(
            scene
                .world
                .all_components()
                .filter(|&id| scene.world.component_label(id) == Some("store_fixture"))
                .count(),
            1
        );
        assert!(
            scene
                .world
                .find_component(store, "#accordion_body")
                .is_some()
        );
    }
    // Exercise the actual grocery store too: its nested mesh/texture factories
    // must work in a live restore callback, not only during initial evaluation.
    let retail = named(&scene.world, "retail_test_accordion");
    let retail_toggle = scene
        .world
        .find_component(retail, "#accordion_toggle")
        .unwrap();
    let shelves = |scene: &Universe| {
        scene
            .world
            .all_components()
            .filter(|&id| scene.world.component_label(id) == Some("retail_shelf"))
            .count()
    };
    assert_eq!(shelves(&scene), 5);
    click(&mut scene, retail_toggle);
    assert_eq!(shelves(&scene), 0);
    click(&mut scene, retail_toggle);
    assert_eq!(shelves(&scene), 5);
    assert!(
        !scene
            .systems
            .rx
            .has_global_handlers(mittens_engine::engine::ecs::SignalKind::FrameTick),
        "idle controller must not request frames"
    );
    assert!(
        !scene
            .systems
            .rx
            .has_global_handlers(mittens_engine::engine::ecs::SignalKind::XrAxisChanged),
        "idle vehicle axes should have no binding"
    );
    let truck = named(&scene.world, "pickup_truck");
    let mount = named(&scene.world, "pickup_entry_zone");
    assert!(
        scene
            .world
            .get_component_by_id_as::<ZoneComponent>(mount)
            .is_some()
    );
    let mut pending = vec![truck];
    let mut hit = None;
    while let Some(id) = pending.pop() {
        pending.extend_from_slice(scene.world.children_of(id));
        if scene
            .world
            .get_component_by_id_as::<RenderableComponent>(id)
            .is_some()
        {
            hit = Some(id);
            break;
        }
    }
    let pointer = named(&scene.world, "test_rider_pointer");
    assert!(
        scene.systems.attachment.try_mount_from_hit(
            &mut scene.world,
            pointer,
            hit.unwrap(),
            &mut scene.command_queue
        ),
        "truck must be enterable through its driver-side zone"
    );
    tick(&mut scene, 3);
    assert!(
        scene
            .systems
            .rx
            .has_global_handlers(mittens_engine::engine::ecs::SignalKind::FrameTick)
    );
    assert!(
        scene
            .systems
            .rx
            .has_global_handlers(mittens_engine::engine::ecs::SignalKind::XrAxisChanged)
    );
    let root = named(&scene.world, "test_rider_root");
    assert!(
        !scene
            .world
            .get_component_by_id_as::<InputComponent>(named(&scene.world, "test_rider_input"))
            .unwrap()
            .translation_enabled
    );
    key(&mut scene, root, "KeyW", true);
    tick(&mut scene, 30);
    let forward = TransformSystem::world_position(&scene.world, truck).unwrap();
    assert!(forward[2] < -1.0, "W should drive forward: {forward:?}");
    key(&mut scene, root, "KeyD", true);
    tick(&mut scene, 20);
    let turned = TransformSystem::world_position(&scene.world, truck).unwrap();
    assert!(
        (turned[0] - forward[0]).abs() > 0.1,
        "steering should change the path"
    );
    key(&mut scene, root, "KeyW", false);
    key(&mut scene, root, "Space", true);
    tick(&mut scene, 8);
    let stopped = TransformSystem::world_position(&scene.world, truck).unwrap();
    tick(&mut scene, 8);
    let still = TransformSystem::world_position(&scene.world, truck).unwrap();
    assert!(
        (still[0] - stopped[0]).abs() + (still[2] - stopped[2]).abs() < 0.02,
        "brake must stop the truck"
    );
    key(&mut scene, root, "Space", false);
    key(&mut scene, root, "KeyD", false);
    scene.command_queue.push_event(
        root,
        EventSignal::XrAxisChanged {
            source_component: named(&scene.world, "test_rider_input"),
            hand: ControllerHand::Left,
            control: XrAxisControl::LeftStick,
            value: [-0.5, 1.0],
        },
    );
    tick(&mut scene, 20);
    let stick_driven = TransformSystem::world_position(&scene.world, truck).unwrap();
    assert!(
        (stick_driven[0] - still[0]).abs() + (stick_driven[2] - still[2]).abs() > 0.1,
        "XR stick should drive the mounted pickup"
    );
    assert!(scene.systems.attachment.try_dismount_for_pointer(
        &mut scene.world,
        pointer,
        &mut scene.command_queue
    ));
    tick(&mut scene, 3);
    let restored = scene
        .world
        .get_component_by_id_as::<InputComponent>(named(&scene.world, "test_rider_input"))
        .unwrap();
    assert!(restored.enabled && restored.translation_enabled);
    assert!(
        !scene
            .systems
            .rx
            .has_global_handlers(mittens_engine::engine::ecs::SignalKind::FrameTick),
        "dismount must bypass frames"
    );
    assert!(
        !scene
            .systems
            .rx
            .has_global_handlers(mittens_engine::engine::ecs::SignalKind::XrAxisChanged),
        "dismount bypasses vehicle axis input"
    );
    // Streaming out the mounted truck must retire the controller's cached
    // driver instead of continuing to invoke a removed vehicle every frame.
    assert!(scene.systems.attachment.try_mount_from_hit(
        &mut scene.world,
        pointer,
        hit.unwrap(),
        &mut scene.command_queue
    ));
    tick(&mut scene, 3);
    let velocity = named(&scene.world, "pickup_velocity");
    scene.command_queue.push_intent_now(
        velocity,
        mittens_engine::engine::ecs::IntentValue::RemoveSubtree {
            component_id: velocity,
        },
    );
    tick(&mut scene, 8);
    assert!(scene.world.get_component_record(truck).is_none());
    assert!(
        !scene
            .systems
            .rx
            .has_global_handlers(mittens_engine::engine::ecs::SignalKind::FrameTick),
        "removed truck must leave frame delivery bypassed"
    );
    assert!(
        !scene
            .systems
            .rx
            .has_global_handlers(mittens_engine::engine::ecs::SignalKind::XrAxisChanged)
    );
    let input = scene
        .world
        .get_component_by_id_as::<InputComponent>(named(&scene.world, "test_rider_input"))
        .unwrap();
    assert!(
        input.enabled && input.translation_enabled,
        "removing a mounted vehicle restores rider input"
    );
}
