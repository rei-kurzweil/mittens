//! The sakura twilight fork of the rooftop example loads without its train.
use crate::engine::Universe;
use crate::engine::ecs::component::{DestroyableComponent, TransformComponent, VelocityComponent};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter, World};

fn named(scene: &Universe, label: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(label))
        .unwrap_or_else(|| panic!("missing {label}"))
}

#[test]
fn sakura_twilight_loads_its_city_without_the_train() {
    let path = "examples/sakura-twilight.mms";
    let mut source = std::fs::read_to_string(path)
        .unwrap()
        .replace("let use_xr = true", "let use_xr = false")
        .replace("let use_desktop = false", "let use_desktop = true");
    source.push_str(
        r##"
        let ramp_test = T { name = "ramp_test_driver" }
        on(ramp_test, "DataEvent", fn(event) {
            if event != "DriveRamp" { return }
            let truck = pickup.query("#pickup_truck")
            let yaw = Math.atan2(-37.8, 12.6)
            truck.set_translation([roof_x + 3.0, roof_height, ramp_entry_z + 3.2/3.0], "world")
            truck.set_rotation_quat([0, Math.sin(yaw/2), 0, Math.cos(yaw/2)], "world")
            truck.set_powered(true)
            truck.drive_command(1,0,false)
        })
    "##,
    );
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(&source, path).unwrap();
    for _ in 0..3 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    let labels: Vec<&str> = scene
        .world
        .all_components()
        .filter_map(|id| scene.world.component_label(id))
        .collect();
    assert!(
        !labels.iter().any(|label| label.contains("train")),
        "no train parts"
    );
    for name in [
        "sakura_twilight_main",
        "sakura_twilight_row_0",
        "sakura_twilight_row_1",
        "sakura_twilight_row_2",
        "sakura_twilight_row_3",
        "sakura_twilight_road_dashes",
        "sakura_twilight_pickup",
        "sakura_twilight_road_ramp",
        "sakura_twilight_row_sta_launcher_grip",
        "sakura_twilight_row_sta_ammo_cabinet",
        "kawaii_twilight_sky",
    ] {
        assert_eq!(
            labels.iter().filter(|&&label| label == name).count(),
            1,
            "{name}"
        );
    }
    for (building, floors) in [
        ("main", 3),
        ("row_0", 2),
        ("row_1", 2),
        ("row_2", 2),
        ("row_3", 1),
    ] {
        let sections = (0..floors)
            .map(|floor| format!("floor_{floor}"))
            .chain(["roof".into()]);
        for section in sections {
            let section = named(&scene, &format!("sakura_twilight_{building}_{section}"));
            assert!(scene.world.children_of(section).iter().any(|&child| {
                scene
                    .world
                    .get_component_by_id_as::<DestroyableComponent>(child)
                    .is_some()
            }));
        }
    }
    scene.command_queue.push_event(
        named(&scene, "ramp_test_driver"),
        EventSignal::DataEvent {
            name: "DriveRamp".into(),
            payload: None,
        },
    );
    let pickup = named(&scene, "pickup_truck");
    let mut position = [0.0; 3];
    for _ in 0..690 {
        scene.update(1.0 / 60.0, &Default::default());
        position = TransformSystem::world_position(&scene.world, pickup).unwrap();
        if position[0] >= 26.0 && position[1] < 0.1 {
            break;
        }
    }
    assert!(
        position[0] >= 26.0 && (position[1] + 0.23).abs() < 0.3,
        "pickup should reach the road from its roof: {position:?}"
    );
}

#[test]
fn sakura_twilight_pickup_coasts_and_follows_an_incline() {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            r##"
        import { pickup_truck } from "../assets/components/vehicles/pickup_truck.mms"
        let motion = pickup_truck(null, null, true, {acceleration_drive = true})
        T { motion }
        T.position(0, -0.15, 1).scale(16, 0.30, 10) {
            Zone.cube([0.5,0.5,0.5]) { Collidable.static().friction(0.55) {} }
        }
        T.position(0, -1, -9).rotation(-Math.atan2(2,10), 0, 0) {
            T.position(0,-0.15,0).scale(16,0.30,Math.sqrt(104)) {
                Zone.cube([0.5,0.5,0.5]) { Collidable.static().friction(0.55) {} }
            }
        }
        T.position(0, -2.15, -20).scale(16, 0.30, 12) {
            Zone.cube([0.5,0.5,0.5]) { Collidable.static().friction(0.55) {} }
        }
        let control = T { name = "test_driver" }
        on(control, "DataEvent", fn(event) {
            let truck = motion.query("#pickup_truck")
            truck.set_powered(true)
            if event == "Drive" { truck.drive_command(1,0,false) }
            if event == "Coast" { truck.drive_command(0,0,false) }
            if event == "Steer" { truck.drive_command(0,0.5,false) }
        })
    "##,
            "examples/_sakura_twilight_pickup_test.mms",
        )
        .unwrap();
    fn tick(scene: &mut Universe, frames: usize) {
        for _ in 0..frames {
            scene.update(1.0 / 60.0, &Default::default());
        }
    }
    fn command(scene: &mut Universe, name: &str) {
        scene.command_queue.push_event(
            named(scene, "test_driver"),
            EventSignal::DataEvent {
                name: name.into(),
                payload: None,
            },
        );
    }
    fn speed(scene: &Universe) -> f32 {
        let velocity = named(scene, "pickup_velocity");
        -scene
            .world
            .get_component_by_id_as::<VelocityComponent>(velocity)
            .unwrap()
            .linear_local_mps[2]
    }
    fn rotation(scene: &Universe, label: &str) -> [f32; 4] {
        scene
            .world
            .get_component_by_id_as::<TransformComponent>(named(scene, label))
            .unwrap()
            .transform
            .rotation
    }
    tick(&mut scene, 6);
    command(&mut scene, "Steer");
    tick(&mut scene, 4);
    assert!(rotation(&scene, "steering_fl")[1].abs() > 0.05);
    assert_eq!(rotation(&scene, "steering_rl"), [0.0, 0.0, 0.0, 1.0]);
    command(&mut scene, "Drive");
    tick(&mut scene, 20);
    let driven = speed(&scene);
    assert!(driven > 1.0 && driven < 5.0, "forward speed: {driven}");
    assert!(rotation(&scene, "wheel_fl")[0].abs() > 0.02);
    command(&mut scene, "Coast");
    tick(&mut scene, 8);
    let coasting = speed(&scene);
    assert!(
        coasting > 0.3 && coasting < driven,
        "coasting: {coasting} from {driven}"
    );
    command(&mut scene, "Drive");
    tick(&mut scene, 105);
    let chassis = named(&scene, "pickup_truck");
    let position = TransformSystem::world_position(&scene.world, chassis).unwrap();
    assert!(
        position[2] < -5.0 && position[1] < -0.15 && position[1] > -2.0,
        "ramp pose: {position:?}"
    );
    assert!(
        rotation(&scene, "pickup_truck")[0] < -0.035,
        "truck should pitch down the ramp"
    );
}
