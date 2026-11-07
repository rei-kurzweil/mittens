use mittens_engine::engine::{
    Universe,
    ecs::{
        ComponentId, EventSignal, SignalEmitter, SignalKind, component::*, system::TransformSystem,
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[derive(Default)]
struct TestClock(AtomicU64);
impl mittens_engine::engine::ecs::system::ClockDriver for TestClock {
    fn name(&self) -> &'static str {
        "pickup-power-test"
    }
    fn time_now_sec(&self) -> f64 {
        self.0.load(Ordering::Relaxed) as f64 / 1_000_000.
    }
}
fn named(scene: &Universe, label: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(label))
        .unwrap_or_else(|| panic!("missing {label}"))
}
fn tick(scene: &mut Universe, clock: &TestClock, frames: usize) {
    for _ in 0..frames {
        clock.0.fetch_add(16_667, Ordering::Relaxed);
        scene.update(1. / 60., &Default::default());
    }
}
fn fixture() -> (Universe, Arc<TestClock>) {
    let mut scene = Universe::new(Default::default());
    let clock = Arc::new(TestClock::default());
    scene.systems.clock.set_driver(clock.clone());
    scene.load_mms_source_at_path(r##"
        import { ship_input_controller } from "../assets/components/ship_input_controller.mms"
        import { pickup_truck } from "../assets/components/vehicles/pickup_truck.mms"
        ship_input_controller(false)
        let vehicle=pickup_truck()
        T.position(0,3,0) {name="fixture" vehicle}
        let floor=T.position(0,-0.1,0).scale(200,0.2,200) {Zone.cube([0.5,0.5,0.5]) {Collidable.static()}}
        floor
        T.position(40,0,0) {name="ship_desktop_camera_rig"}
        let control=T {name="control"}
        on(control,"DataEvent",fn(event) {
            let truck=vehicle.query("#pickup_truck")
            if event=="Drive" {truck.drive_command(0.5,0,false)}
            if event=="DropFloor" {floor.remove_subtree()}
            if event=="Kick" {vehicle.translate_world([1,3,0]) vehicle.rotate_world([0,0,0.5])}
        })
    "##, "examples/_pickup_power_test.mms").unwrap();
    (scene, clock)
}
fn command(scene: &mut Universe, clock: &TestClock, name: &str) {
    scene.command_queue.push_event(
        named(scene, "control"),
        EventSignal::DataEvent {
            name: name.into(),
            payload: None,
        },
    );
    tick(scene, clock, 1);
}
fn click_power(scene: &mut Universe, clock: &TestClock) {
    let button = named(scene, "pickup_power_button");
    scene.command_queue.push_event(
        button,
        EventSignal::Click {
            raycaster: ComponentId::default(),
            renderable: button,
            hit_point: [0.; 3],
            screen_pos_px: None,
        },
    );
    tick(scene, clock, 2);
}
fn velocity(scene: &Universe) -> &VelocityComponent {
    scene
        .world
        .get_component_by_id_as::<VelocityComponent>(named(scene, "pickup_velocity"))
        .unwrap()
}
fn position(scene: &Universe) -> [f32; 3] {
    TransformSystem::world_position(&scene.world, named(scene, "pickup_truck")).unwrap()
}
fn label(scene: &Universe) -> &str {
    &scene
        .world
        .get_component_by_id_as::<TextComponent>(named(scene, "pickup_power_readout"))
        .unwrap()
        .text
}
fn assert_physics_and_no_polling(scene: &Universe) {
    assert!(
        scene
            .world
            .get_component_by_id_as::<GravityComponent>(named(scene, "pickup_gravity"))
            .unwrap()
            .enabled
    );
    assert!(velocity(scene).enabled);
    for name in [
        "pickup_parking_check",
        "pickup_activity_inner",
        "pickup_activity_outer",
    ] {
        assert!(
            !scene
                .world
                .all_components()
                .any(|id| scene.world.component_label(id) == Some(name)),
            "retired {name}"
        );
    }
    assert!(!scene.systems.rx.has_global_handlers(SignalKind::FrameTick));
    assert!(
        !scene
            .systems
            .rx
            .has_global_handlers(SignalKind::XrAxisChanged)
    );
}
#[test]
fn power_button_is_reachable_unmounted_and_off_preserves_motion() {
    let (mut scene, clock) = fixture();
    tick(&mut scene, &clock, 10);
    assert!(position(&scene)[1] < 3. && position(&scene)[1] > 1.);
    assert_eq!(label(&scene), "POWER OFF");
    assert_physics_and_no_polling(&scene);
    tick(&mut scene, &clock, 120);
    assert!(position(&scene)[1].abs() < 0.05);
    assert_physics_and_no_polling(&scene);
    // Proximity no longer governs either control power or physics.
    let player = named(&scene, "ship_desktop_camera_rig");
    for x in [0., 40.] {
        let transform = scene
            .world
            .get_component_by_id_as_mut::<TransformComponent>(player)
            .unwrap();
        transform.transform.translation[0] = x;
        transform.transform.recompute_model();
        scene
            .systems
            .transform_changed(&mut scene.world, &mut scene.visuals, player);
        tick(&mut scene, &clock, 40);
        assert_eq!(label(&scene), "POWER OFF");
        assert_physics_and_no_polling(&scene);
    }
    command(&mut scene, &clock, "Drive");
    assert_eq!(
        velocity(&scene).linear_local_mps[2],
        0.,
        "NPC commands cannot turn power on"
    );
    click_power(&mut scene, &clock);
    assert_eq!(label(&scene), "POWER ON");
    assert_physics_and_no_polling(&scene); // Nobody mounted: no driver callbacks needed.
    command(&mut scene, &clock, "Drive");
    assert!(velocity(&scene).linear_local_mps[2] < -2.);
    click_power(&mut scene, &clock);
    assert_eq!(label(&scene), "POWER OFF");
    assert!(
        velocity(&scene).linear_local_mps[2] < -2.,
        "power off must not erase momentum"
    );
    let before = position(&scene);
    tick(&mut scene, &clock, 30);
    assert!(position(&scene)[2] < before[2] - 0.5);
    assert_physics_and_no_polling(&scene);
}
#[test]
fn off_truck_loses_support_and_accepts_external_linear_and_angular_motion() {
    let (mut scene, clock) = fixture();
    tick(&mut scene, &clock, 140);
    let settled = position(&scene);
    assert!(settled[1].abs() < 0.05);
    command(&mut scene, &clock, "DropFloor");
    tick(&mut scene, &clock, 20);
    assert!(
        position(&scene)[1] < settled[1] - 0.1,
        "support removal must wake/fall without player proximity"
    );
    command(&mut scene, &clock, "Kick");
    let before = position(&scene);
    let truck = named(&scene, "pickup_truck");
    let rotation = scene
        .world
        .get_component_by_id_as::<TransformComponent>(truck)
        .unwrap()
        .transform
        .rotation;
    tick(&mut scene, &clock, 10);
    assert!(position(&scene)[0] > before[0] + 0.1);
    assert_ne!(
        scene
            .world
            .get_component_by_id_as::<TransformComponent>(truck)
            .unwrap()
            .transform
            .rotation,
        rotation
    );
    assert_eq!(label(&scene), "POWER OFF");
    assert_physics_and_no_polling(&scene);
}
