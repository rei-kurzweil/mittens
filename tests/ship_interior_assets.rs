use mittens_engine::engine::{
    Universe,
    ecs::{
        ComponentId, EventSignal, KeyboardEvent, SignalEmitter, SignalKind, World,
        component::{
            CombineMeshComponent, DestroyableComponent, GrabbableComponent, RenderableComponent,
            ZoneComponent,
        },
    },
    graphics::primitives::CpuMeshHandle,
};

#[derive(Default)]
struct Upload(u32);
impl mittens_engine::engine::graphics::MeshUploader for Upload {
    fn upload_mesh(
        &mut self,
        _: &mittens_engine::engine::graphics::CpuMesh,
    ) -> Result<mittens_engine::engine::graphics::MeshHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(mittens_engine::engine::graphics::MeshHandle(self.0))
    }
}
impl mittens_engine::engine::graphics::TextureUploader for Upload {
    fn upload_texture_rgba8(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
    ) -> Result<mittens_engine::engine::graphics::TextureHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(mittens_engine::engine::graphics::TextureHandle(self.0))
    }
    fn upload_texture_bc7(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
        _: bool,
    ) -> Result<mittens_engine::engine::graphics::TextureHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(mittens_engine::engine::graphics::TextureHandle(self.0))
    }
}

#[test]
fn ship_vehicles_and_barracks_preserve_damage_and_weapon_topology() {
    // One scene avoids concurrent native shader preparation in this test binary.
    let mut scene = Universe::new(World::default());
    scene
        .systems
        .xr
        .set_external_frame(Some(Default::default()));
    scene.load_mms_source_at_path(r#"
        import { ship_input_controller } from "../assets/components/ship_input_controller.mms"
        ship_input_controller(false)
        import { recreational_vehicle } from "../assets/components/vehicles/recreational_vehicle.mms"
        import { pickup_truck } from "../assets/components/vehicles/pickup_truck.mms"
        import { motorcycle } from "../assets/components/vehicles/motorcycle.mms"
        import { hot_dog_stand } from "../assets/components/hot_dog_stand.mms"
        import { ship_barracks } from "../assets/components/rooms/ship_barracks.mms"
        recreational_vehicle(0.9)
        T.position(8,0,0) { pickup_truck(null) }
        T.position(12,0,0) { motorcycle(null) }
        T.position(20,0,0) { ship_barracks() }
        T.position(30,0,0) { hot_dog_stand() }
    "#, "examples/_ship_assets_test.mms").unwrap();
    // Custom weapon effects start native compiler workers. Let their retained
    // definitions finish before this short-lived headless process shuts down.
    let definitions: Vec<_> = scene
        .world
        .all_components()
        .filter_map(|source| {
            let registry = &scene.visuals.materials;
            registry
                .definition(registry.source_definition(source)?)?
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
            "material preparation did not finish"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    for definition in &definitions {
        assert!(
            definition.compiled.get().unwrap().is_ok(),
            "custom material failed to compile"
        );
    }
    let mut upload = Upload::default();
    for _ in 0..4 {
        scene.update(1.0 / 90.0, &Default::default());
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut upload,
            &mut scene.command_queue,
        );
    }
    assert_hot_dog_stand(&scene.world);
    assert_cabin_and_window_openings(&scene.world);
    assert_vehicle_damage_topology(&scene.world);
    assert_barracks_stock_and_weapons(&scene.world);
    assert_controller_routes_held_weapon(&mut scene);
}

fn assert_vehicle_damage_topology(world: &World) {
    for label in ["recreational_vehicle", "pickup_truck", "motorcycle"] {
        let root = world
            .all_components()
            .find(|&id| world.component_label(id) == Some(label))
            .expect("vehicle factory emitted its root");
        assert!(world.children_of(root).iter().any(|&id| {
            world
                .get_component_by_id_as::<DestroyableComponent>(id)
                .is_some()
        }));
        let batch = world
            .children_of(root)
            .iter()
            .copied()
            .find(|&id| {
                world
                    .get_component_by_id_as::<CombineMeshComponent>(id)
                    .is_some_and(|batch| batch.keep_transforms)
            })
            .expect("retained body batch supports unpacking on damage");
        let mut surfaces = 0;
        let mut pending = vec![batch];
        while let Some(source) = pending.pop() {
            pending.extend_from_slice(world.children_of(source));
            if !world
                .get_component_by_id_as::<RenderableComponent>(source)
                .is_some_and(|render| render.renderable.base_mesh == CpuMeshHandle::CUBE)
            {
                continue;
            }
            let frame = world.parent_of(source).unwrap();
            // Non-collidable light trim is excluded; body panels must remain
            // discoverable as the direct cube/zone siblings required by CSG.
            if world
                .children_of(frame)
                .iter()
                .any(|&id| world.get_component_by_id_as::<ZoneComponent>(id).is_some())
            {
                surfaces += 1;
            }
        }
        assert!(surfaces >= 7, "{label} has too few cuttable body panels");
    }
}

fn assert_barracks_stock_and_weapons(world: &World) {
    for index in 0..3 {
        let label = format!("ship_rack_pistol_{index}_grip");
        let grip = world
            .all_components()
            .find(|&id| world.component_label(id) == Some(label.as_str()))
            .expect("each pistol has its own grip and handler scope");
        assert!(world.children_of(grip).iter().any(|&id| {
            world
                .get_component_by_id_as::<GrabbableComponent>(id)
                .is_some()
        }));
    }
    assert_eq!(
        world
            .all_components()
            .filter(|&id| world.component_label(id) == Some("retail_shelf"))
            .count(),
        5
    );
}

fn assert_hot_dog_stand(world: &World) {
    let named = |label: &str| {
        world
            .all_components()
            .find(|&id| world.component_label(id) == Some(label))
            .unwrap()
    };
    for index in 0..4 {
        named(&format!("fountain_drink_nozzle_{index}"));
    }
    named("ship_hot_dog_vendor");
    let held = named("ship_vendor_held_hot_dog");
    assert_eq!(
        world.component_label(world.parent_of(held).unwrap()),
        Some("J_Bip_R_Hand")
    );
    named("hot_dog_grill");
    let fridge = named("mini_fridge");
    assert_eq!(
        world.component_label(world.parent_of(fridge).unwrap()),
        Some("pickup_bed_cargo_mount")
    );
}

fn assert_cabin_and_window_openings(world: &World) {
    for label in [
        "pickup_front_seat_-1",
        "pickup_front_seat_1",
        "pickup_steering_wheel",
        "pickup_accelerator_pedal",
        "pickup_brake_pedal",
        "pickup_gear_shift",
        "pickup_radio_recess",
        "pickup_gauge_rpm",
        "pickup_gauge_speed",
        "pickup_gauge_fuel",
        "pickup_gauge_odometer",
        "pickup_dashboard_mount_0",
        "pickup_dashboard_mount_1",
        "pickup_dashboard_mount_2",
    ] {
        assert!(
            world
                .all_components()
                .any(|id| world.component_label(id) == Some(label)),
            "missing cab fixture {label}"
        );
    }
    // A driver's torso and an RV side window must occupy real open space,
    // rather than sitting inside the old solid cab/side-wall cubes.
    for point in [[7.52, 1.45, -0.05], [1.40, 2.12, 2.25]] {
        for id in world.all_components() {
            if !world
                .get_component_by_id_as::<RenderableComponent>(id)
                .is_some_and(|r| r.renderable.base_mesh == CpuMeshHandle::CUBE)
            {
                continue;
            }
            let m = mittens_engine::engine::ecs::system::TransformSystem::world_model(world, id)
                .unwrap();
            let inside = (0..3).all(|axis| {
                (point[axis] - m[3][axis]).abs()
                    < (0..3).map(|i| m[i][axis].abs() * 0.5).sum::<f32>() - 0.005
            });
            assert!(
                !inside,
                "opaque panel {id:?} fills cab/window opening at {point:?}"
            );
        }
    }
}

// Exercise the persistent input route against a real weapon, then unload it.
fn assert_controller_routes_held_weapon(scene: &mut Universe) {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let named = |scene: &Universe, label: &str| {
        scene
            .world
            .all_components()
            .find(|&id| scene.world.component_label(id) == Some(label))
            .unwrap()
    };
    let grip = named(scene, "ship_rack_pistol_0_grip");
    let completion = named(scene, "ship_rack_pistol_0_laser_completion");
    let shots = Arc::new(AtomicUsize::new(0));
    let recorded = shots.clone();
    scene
        .systems
        .rx
        .add_handler_closure(SignalKind::DataEvent, completion, move |_, _, signal| {
            if matches!(&signal.event,Some(EventSignal::DataEvent{name,..}) if name=="ShotFired") {
                recorded.fetch_add(1, Ordering::Relaxed);
            }
        });
    let tick = |scene: &mut Universe| {
        for _ in 0..8 {
            scene.update(0.025, &Default::default());
        }
    };
    let key = |scene: &mut Universe, down| {
        let event = KeyboardEvent {
            code: Some("Space".into()),
            key: " ".into(),
        };
        scene.command_queue.push_event(
            ComponentId::default(),
            if down {
                EventSignal::KeyDown(event)
            } else {
                EventSignal::KeyUp(event)
            },
        );
    };
    key(scene, true);
    tick(scene);
    assert_eq!(
        shots.load(Ordering::Relaxed),
        0,
        "rack weapons must ignore fire input"
    );
    key(scene, false);
    tick(scene);
    scene.command_queue.push_event(
        grip,
        EventSignal::GrabStart {
            pointer: ComponentId::default(),
            raycaster: ComponentId::default(),
            renderable: grip,
            target: grip,
            ray_origin_world: [0.; 3],
            ray_dir_world: [0., 0., -1.],
        },
    );
    tick(scene);
    key(scene, true);
    tick(scene);
    assert_eq!(
        shots.load(Ordering::Relaxed),
        1,
        "controller must fire the held weapon once"
    );
    key(scene, false);
    tick(scene);
    scene.command_queue.push_event(
        grip,
        EventSignal::GrabEnd {
            pointer: ComponentId::default(),
            target: grip,
        },
    );
    tick(scene);
    key(scene, true);
    tick(scene);
    assert_eq!(
        shots.load(Ordering::Relaxed),
        1,
        "released weapon must ignore input"
    );
    // Removal must not leave a global handler trying to call the old grip.
    scene.command_queue.push_intent_now(
        ComponentId::default(),
        mittens_engine::engine::ecs::IntentValue::RemoveSubtree { component_id: grip },
    );
    tick(scene);
    key(scene, false);
    tick(scene);
    assert!(scene.world.get_component_record(grip).is_none());
}
