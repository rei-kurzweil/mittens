use mittens_engine::engine::{
    Universe,
    ecs::{ComponentId, EventSignal, SignalEmitter, component::*, system::TransformSystem},
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[derive(Default)]
struct TestClock(AtomicU64);
impl mittens_engine::engine::ecs::system::ClockDriver for TestClock {
    fn name(&self) -> &'static str {
        "pickup-activity-test"
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
fn gravity(scene: &Universe) -> bool {
    scene
        .world
        .get_component_by_id_as::<GravityComponent>(named(scene, "pickup_gravity"))
        .unwrap()
        .enabled
}
fn move_player(scene: &mut Universe, x: f32) {
    let id = named(scene, "ship_desktop_camera_rig");
    let transform = scene
        .world
        .get_component_by_id_as_mut::<TransformComponent>(id)
        .unwrap();
    transform.transform.translation = [x, 0., 0.];
    transform.transform.recompute_model();
    scene
        .systems
        .transform_changed(&mut scene.world, &mut scene.visuals, id);
}
#[test]
fn proximity_parks_supported_truck_wakes_with_deadband_and_pins_npc_commands() {
    let mut scene = Universe::new(Default::default());
    let clock = Arc::new(TestClock::default());
    scene.systems.clock.set_driver(clock.clone());
    scene.systems.clock.set_bpm(60.);
    scene
        .load_mms_source_at_path(
            r##"
        import { ship_input_controller } from "../assets/components/ship_input_controller.mms"
        import { pickup_truck } from "../assets/components/vehicles/pickup_truck.mms"
        ship_input_controller(false)
        let vehicle=pickup_truck(null,null,null,true)
        T.position(0,3,0) {name="fixture" vehicle}
        T.position(0,-0.1,0).scale(200,0.2,200) {Zone.cube([0.5,0.5,0.5]) {Collidable.static()}}
        T.position(40,0,0) {
            name="ship_desktop_camera_rig"
            T.position(0,0.9,0) {Zone.capsule_y(0.25,0.55) {
                Collidable.slide().movement_target("/#ship_desktop_camera_rig")
            }}
        }
        let control=T {name="control"}
        on(control,"DataEvent",fn(event) {
            let truck=vehicle.query("#pickup_truck")
            if event=="Drive" {truck.drive_command(0.5,0,false)}
            if event=="Stop" {truck.drive_command(0,0,true)}
            if event=="Off" {truck.set_active(false)}
            if event=="On" {truck.set_active(true)}
            if event=="Auto" {truck.use_proximity_activation()}
        })
    "##,
            "examples/_pickup_activity_test.mms",
        )
        .unwrap();
    let truck = named(&scene, "pickup_truck");
    tick(&mut scene, &clock, 10);
    let falling = TransformSystem::world_position(&scene.world, truck).unwrap();
    assert!(
        gravity(&scene) && falling[1] > 1.,
        "distant truck must fall before parking: {falling:?}"
    );
    tick(&mut scene, &clock, 120);
    let parked = TransformSystem::world_position(&scene.world, truck).unwrap();
    assert!(!gravity(&scene), "supported distant truck should park");
    assert!(
        parked[1].abs() < 0.05,
        "truck must reach deck before parking: {parked:?}"
    );
    assert!(
        !scene
            .world
            .all_components()
            .any(|id| scene.world.component_label(id) == Some("pickup_parking_check")),
        "parked truck must have no polling animation"
    );
    tick(&mut scene, &clock, 60);
    assert_eq!(
        TransformSystem::world_position(&scene.world, truck).unwrap(),
        parked
    );
    move_player(&mut scene, 4.);
    tick(&mut scene, &clock, 2);
    assert!(gravity(&scene), "inner sensor wakes player-near truck");
    move_player(&mut scene, 6.);
    tick(&mut scene, &clock, 40);
    assert!(
        gravity(&scene),
        "outer sensor retains active state in deadband"
    );
    move_player(&mut scene, 10.);
    tick(&mut scene, &clock, 60);
    assert!(
        !gravity(&scene),
        "leaving outer sensor parks truck after contact"
    );
    command(&mut scene, &clock, "Drive");
    let initial = TransformSystem::world_position(&scene.world, truck).unwrap();
    tick(&mut scene, &clock, 60);
    let driven = TransformSystem::world_position(&scene.world, truck).unwrap();
    assert!(
        gravity(&scene) && driven[2] < initial[2] - 1.,
        "NPC command wakes and pins truck without a nearby player"
    );
    command(&mut scene, &clock, "Stop");
    tick(&mut scene, &clock, 60);
    assert!(!gravity(&scene), "stopping NPC releases active pin");
    command(&mut scene, &clock, "On");
    tick(&mut scene, &clock, 60);
    assert!(gravity(&scene), "explicit on keeps distant truck active");
    command(&mut scene, &clock, "Off");
    tick(&mut scene, &clock, 60);
    assert!(!gravity(&scene), "explicit off parks truck");
    move_player(&mut scene, 4.);
    tick(&mut scene, &clock, 30);
    assert!(!gravity(&scene), "manual off overrides proximity");
    command(&mut scene, &clock, "Auto");
    tick(&mut scene, &clock, 2);
    assert!(gravity(&scene), "auto restores proximity policy");
}
