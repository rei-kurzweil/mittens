use mittens_engine::engine::{
    Universe,
    ecs::{ComponentId, EventSignal, SignalEmitter, World, system::TransformSystem},
};

fn named(world: &World, label: &str) -> ComponentId {
    world
        .all_components()
        .find(|&id| world.component_label(id) == Some(label))
        .unwrap()
}
fn position(scene: &Universe, label: &str) -> [f32; 3] {
    let fixture = named(&scene.world, label);
    let truck = scene
        .world
        .find_component(fixture, "#pickup_truck")
        .unwrap();
    TransformSystem::world_position(&scene.world, truck).unwrap()
}
fn tick(scene: &mut Universe, frames: usize) {
    for _ in 0..frames {
        scene.update(1.0 / 60.0, &Default::default());
    }
}
fn command(scene: &mut Universe, name: &str) {
    scene.command_queue.push_event(
        named(&scene.world, "test_control"),
        EventSignal::DataEvent {
            name: name.into(),
            payload: None,
        },
    );
}

#[test]
fn rotated_pickup_slides_along_wall_without_self_collision_or_moving_other_truck() {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            r##"
        import { pickup_truck } from "../assets/components/vehicles/pickup_truck.mms"
        let first=pickup_truck()
        T.rotation(0,Math.pi/4,0) {name="moving_fixture" first}
        T.position(10,0,0).rotation(0,-Math.pi/4,0) {name="parked_fixture" pickup_truck()}
        // Frictionless long wall isolates the slide response from braking.
        T.position(-3,1,0).scale(0.2,4,30) {Zone.cube([0.5,0.5,0.5]) {Collidable.static()}}
        T.position(0,-0.1,0).scale(50,0.2,50) {Zone.cube([0.5,0.5,0.5]) {Collidable.static()}}
        let control=T {name="test_control"}
        on(control,"DataEvent",fn(event) {
            let truck=first.query("#pickup_truck")
            if event=="Drive" {truck.set_powered(true) truck.drive_command(1,0,false)}
            if event=="Reverse" {truck.drive_command(-1,0,false)}
            if event=="Stop" {truck.drive_command(0,0,true)}
        })
    "##,
            "examples/_pickup_slide_test.mms",
        )
        .unwrap();
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
    tick(&mut scene, 30);
    let initial = position(&scene, "moving_fixture");
    assert!(
        initial[0].abs() < 1e-5 && initial[2].abs() < 1e-5 && initial[1].abs() < 0.03,
        "own body panels and dashboard must not eject the truck"
    );
    let parked = position(&scene, "parked_fixture");
    command(&mut scene, "Drive");
    tick(&mut scene, 120);
    let contact = position(&scene, "moving_fixture");
    assert!(
        contact[0] > -2.1 && contact[0] < -1.0 && contact[2] < -2.0,
        "truck must stop penetrating the wall: {contact:?}"
    );
    tick(&mut scene, 60);
    let slid = position(&scene, "moving_fixture");
    assert!(
        (slid[0] - contact[0]).abs() < 0.02 && slid[2] < contact[2] - 1.0,
        "tangent motion must continue along the wall: {contact:?} -> {slid:?}"
    );
    let parked_after = position(&scene, "parked_fixture");
    assert!(
        (0..3).all(|i| (parked_after[i] - parked[i]).abs() < 1e-5),
        "each collider must target its own truck"
    );
    command(&mut scene, "Reverse");
    tick(&mut scene, 60);
    assert!(
        position(&scene, "moving_fixture")[0] > slid[0] + 1.0,
        "reversing must leave the contact freely"
    );
}

#[test]
fn pickup_falls_settles_and_braking_does_not_cancel_gravity() {
    use mittens_engine::engine::ecs::component::VelocityComponent;
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            r##"
        import { pickup_truck } from "../assets/components/vehicles/pickup_truck.mms"
        let vehicle=pickup_truck()
        T.position(0,3,0) {name="falling_fixture" vehicle}
        let floor=T.position(0,-0.1,0).scale(30,0.2,30) {
            Zone.cube([0.5,0.5,0.5]) {Collidable.static()}
        }
        floor
        let control=T {name="test_control"}
        on(control,"DataEvent",fn(event) {
            if event=="Brake" {let truck=vehicle.query("#pickup_truck") truck.set_powered(true) truck.drive_command(0,0,true)}
            if event=="DropFloor" {floor.remove_subtree()}
        })
    "##,
            "examples/_pickup_gravity_test.mms",
        )
        .unwrap();
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
    tick(&mut scene, 15);
    let falling = position(&scene, "falling_fixture");
    assert!(
        falling[1] < 2.9 && falling[1] > 2.0,
        "the truck must fall from its spawn height: {falling:?}"
    );
    let velocity = named(&scene.world, "pickup_velocity");
    let before = scene
        .world
        .get_component_by_id_as::<VelocityComponent>(velocity)
        .unwrap()
        .linear_local_mps[1];
    assert!(before < -1.0);
    command(&mut scene, "Brake");
    tick(&mut scene, 1);
    assert!(
        scene
            .world
            .get_component_by_id_as::<VelocityComponent>(velocity)
            .unwrap()
            .linear_local_mps[1]
            <= before,
        "braking should not erase downward velocity"
    );
    tick(&mut scene, 120);
    let settled = position(&scene, "falling_fixture");
    assert!(
        settled[1] > -0.03 && settled[1] < 0.01,
        "truck must settle on the floor, not hover: {settled:?}"
    );
    assert!(
        scene
            .world
            .get_component_by_id_as::<VelocityComponent>(velocity)
            .unwrap()
            .linear_local_mps[1]
            .abs()
            < 1e-4
    );
    command(&mut scene, "DropFloor");
    tick(&mut scene, 20);
    assert!(
        position(&scene, "falling_fixture")[1] < settled[1] - 0.1,
        "removing support should make the parked truck fall again"
    );
}
